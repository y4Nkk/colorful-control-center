//! 已确认操作的同一可执行文件工作进程；不启动窗口或命令解释器。
#[path = "authorization/prompt.rs"]
pub(crate) mod prompt;
#[path = "authorization/session.rs"]
pub(crate) mod session;
use anyhow::{Result, bail};
use windows_sys::Win32::{
    Foundation::*,
    Security::*,
    System::Threading::*,
    UI::{Shell::*, WindowsAndMessaging::SW_HIDE},
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
pub(crate) fn elevated() -> Result<bool> {
    let mut token = std::ptr::null_mut();
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    let mut elevation: TOKEN_ELEVATION = unsafe { std::mem::zeroed() };
    let mut size = 0;
    let ok = unsafe {
        GetTokenInformation(
            token,
            TokenElevation,
            (&mut elevation as *mut TOKEN_ELEVATION).cast(),
            std::mem::size_of_val(&elevation) as u32,
            &mut size,
        )
    };
    let error = std::io::Error::last_os_error();
    unsafe {
        CloseHandle(token);
    }
    if ok == 0 {
        return Err(error.into());
    }
    Ok(elevation.TokenIsElevated != 0)
}
pub fn launch(arguments: &str) -> Result<u32> {
    if arguments.contains('\0') || arguments.encode_utf16().count() > 30_000 {
        bail!("授权工作参数过长或包含无效字符");
    }
    let params = wide(arguments);
    let verb = wide(prompt::verb(elevated()?, prompt::interactive())?);
    let executable = wide(&std::env::current_exe()?.to_string_lossy());
    let mut info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
    info.cbSize = std::mem::size_of_val(&info) as u32;
    info.fMask = SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC;
    info.lpVerb = verb.as_ptr();
    info.lpFile = executable.as_ptr();
    info.lpParameters = params.as_ptr();
    info.nShow = SW_HIDE;
    if unsafe { ShellExecuteExW(&mut info) } == 0 {
        bail!("系统授权未执行：{}", std::io::Error::last_os_error());
    }
    if info.hProcess.is_null() {
        bail!("系统未返回授权工作进程");
    }
    let wait = unsafe { WaitForSingleObject(info.hProcess, INFINITE) };
    let mut code = 0;
    let got = unsafe { GetExitCodeProcess(info.hProcess, &mut code) };
    unsafe {
        CloseHandle(info.hProcess);
    }
    if wait != WAIT_OBJECT_0 || got == 0 {
        bail!("不能确认授权操作结束，请重新读取后恢复");
    }
    Ok(code)
}
