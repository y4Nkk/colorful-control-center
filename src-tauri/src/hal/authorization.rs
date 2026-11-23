//! Windows 令牌检查及同一可执行文件的授权启动。
#[path = "authorization/prompt.rs"]
pub(crate) mod prompt;
#[path = "authorization/session.rs"]
pub(crate) mod session;
use anyhow::{Result, bail};
use std::os::windows::{
    ffi::OsStrExt,
    io::{AsRawHandle, FromRawHandle, OwnedHandle},
};
use windows_sys::Win32::{
    Foundation::*,
    Security::*,
    System::Threading::*,
    UI::{
        Shell::*,
        WindowsAndMessaging::{SW_HIDE, SW_SHOWNORMAL},
    },
};
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}
pub fn require_elevated() -> Result<()> {
    if !elevated()? {
        bail!("硬件控制工作进程需要已确认的管理员授权");
    }
    Ok(())
}
pub(crate) fn current_token(access: TOKEN_ACCESS_MASK) -> Result<OwnedHandle> {
    let mut token = std::ptr::null_mut();
    if unsafe { OpenProcessToken(GetCurrentProcess(), access, &mut token) } == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(unsafe { OwnedHandle::from_raw_handle(token) })
}

pub(crate) fn elevated() -> Result<bool> {
    let token = current_token(TOKEN_QUERY)?;
    token_is_elevated(&token)
}

fn token_is_elevated(token: &OwnedHandle) -> Result<bool> {
    let mut elevation: TOKEN_ELEVATION = unsafe { std::mem::zeroed() };
    let mut size = 0;
    let ok = unsafe {
        GetTokenInformation(
            token.as_raw_handle(),
            TokenElevation,
            (&mut elevation as *mut TOKEN_ELEVATION).cast(),
            std::mem::size_of_val(&elevation) as u32,
            &mut size,
        )
    };
    if ok == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(elevation.TokenIsElevated != 0)
}

pub(crate) const SAME_USER_ADMIN_REQUIRED: &str =
    "当前 Windows 账户没有可用的管理员权限，请使用管理员账户登录 Windows 后打开应用。";

pub(crate) enum AdministrativeAccess {
    Elevated,
    CanElevate,
    Standard,
}

pub(crate) fn administrative_access() -> Result<AdministrativeAccess> {
    let token = current_token(TOKEN_QUERY)?;
    if token_is_elevated(&token)? {
        return Ok(AdministrativeAccess::Elevated);
    }
    let kind = token_elevation_type(&token)?;
    if kind == TokenElevationTypeLimited {
        Ok(AdministrativeAccess::CanElevate)
    } else if kind == TokenElevationTypeDefault {
        Ok(AdministrativeAccess::Standard)
    } else {
        bail!("应用的管理员令牌状态无效");
    }
}

fn token_elevation_type(token: &OwnedHandle) -> Result<TOKEN_ELEVATION_TYPE> {
    let mut kind = 0;
    let mut size = 0;
    if unsafe {
        GetTokenInformation(
            token.as_raw_handle(),
            TokenElevationType,
            (&mut kind as *mut TOKEN_ELEVATION_TYPE).cast(),
            std::mem::size_of_val(&kind) as u32,
            &mut size,
        )
    } == 0
    {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(kind)
}

pub(crate) fn user() -> Result<String> {
    let token = current_token(TOKEN_QUERY)?;
    let mut size = 0;
    unsafe {
        GetTokenInformation(
            token.as_raw_handle(),
            TokenUser,
            std::ptr::null_mut(),
            0,
            &mut size,
        )
    };
    if size == 0 || size > 65536 {
        bail!("当前用户令牌大小无效");
    }
    let mut buffer = vec![0usize; (size as usize).div_ceil(std::mem::size_of::<usize>())];
    if unsafe {
        GetTokenInformation(
            token.as_raw_handle(),
            TokenUser,
            buffer.as_mut_ptr().cast(),
            size,
            &mut size,
        )
    } == 0
    {
        return Err(std::io::Error::last_os_error().into());
    }
    let sid = unsafe { &*(buffer.as_ptr().cast::<TOKEN_USER>()) }.User.Sid;
    let count = unsafe { GetSidSubAuthorityCount(sid) };
    let authority = unsafe { GetSidIdentifierAuthority(sid) };
    if count.is_null() || authority.is_null() {
        bail!("当前用户SID无效");
    }
    let authority = unsafe { (*authority).Value }
        .into_iter()
        .fold(0u64, |v, b| (v << 8) | u64::from(b));
    let mut text = format!("S-1-{authority}");
    for index in 0..unsafe { *count } {
        text.push_str(&format!("-{}", unsafe {
            *GetSidSubAuthority(sid, u32::from(index))
        }));
    }
    Ok(text)
}

pub fn launch(arguments: &str) -> Result<u32> {
    let process = shell_execute(
        prompt::verb(elevated()?, prompt::interactive())?,
        arguments,
        SW_HIDE,
    )?;
    let wait = unsafe { WaitForSingleObject(process.as_raw_handle(), INFINITE) };
    let mut code = 0;
    let got = unsafe { GetExitCodeProcess(process.as_raw_handle(), &mut code) };
    if wait != WAIT_OBJECT_0 || got == 0 {
        bail!("不能确认授权操作结束，请重新读取后恢复");
    }
    Ok(code)
}

pub(crate) fn launch_elevated_window(arguments: &str) -> Result<OwnedHandle> {
    shell_execute("runas", arguments, SW_SHOWNORMAL)
}

fn shell_execute(verb: &str, arguments: &str, show: i32) -> Result<OwnedHandle> {
    if arguments.contains('\0') || arguments.encode_utf16().count() > 30_000 {
        bail!("授权工作参数过长或包含无效字符");
    }
    let params = wide(arguments);
    let verb = wide(verb);
    let executable: Vec<_> = std::env::current_exe()?
        .as_os_str()
        .encode_wide()
        .chain([0])
        .collect();
    let mut info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
    info.cbSize = std::mem::size_of_val(&info) as u32;
    info.fMask = SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC;
    info.lpVerb = verb.as_ptr();
    info.lpFile = executable.as_ptr();
    info.lpParameters = params.as_ptr();
    info.nShow = show;
    if unsafe { ShellExecuteExW(&mut info) } == 0 {
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() == Some(ERROR_CANCELLED as i32) {
            bail!("已取消 Windows 管理员授权");
        }
        bail!("系统授权未执行：{error}");
    }
    if info.hProcess.is_null() {
        bail!("系统未返回授权工作进程");
    }
    Ok(unsafe { OwnedHandle::from_raw_handle(info.hProcess) })
}
