use std::{
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    path::Path,
    ptr::null_mut,
};

use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use windows_sys::{
    Win32::{
        Foundation::{
            ERROR_INSUFFICIENT_BUFFER, FILETIME, HANDLE, HWND, LPARAM, WAIT_OBJECT_0, WAIT_TIMEOUT,
        },
        Storage::Packaging::Appx::{
            GetApplicationUserModelId, GetPackageFullName, GetPackagePathByFullName,
        },
        System::{
            RemoteDesktop::ProcessIdToSessionId,
            Threading::{
                GetProcessTimes, OpenProcess, PROCESS_NAME_WIN32,
                PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE, QueryFullProcessImageNameW,
                WaitForSingleObject,
            },
        },
        UI::WindowsAndMessaging::{
            EnumWindows, GetClassNameW, GetWindowTextW, GetWindowThreadProcessId, PostMessageW,
            WM_CLOSE,
        },
    },
    core::BOOL,
};

use super::{FN_KEY, owners};

const APP_ID: &str = "CLEVOCO.FnhotkeysandOSD_6h6z29zh29qx0!App";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct FnKeyIdentity {
    pid: u32,
    creation_time: [u32; 2],
}

pub(crate) struct FnKeyProcess {
    handle: OwnedHandle,
    identity: FnKeyIdentity,
    window: HWND,
}

impl FnKeyProcess {
    pub(crate) fn blocking() -> Result<Option<Self>> {
        let active = owners()?;
        match active.as_slice() {
            [] => Ok(None),
            [owner] if owner.executable == FN_KEY => Self::open(owner.pid)
                .map(Some)
                .context("无法核验原厂 FnKey 的关闭入口，请从任务管理器退出 FnKey.exe 后重试"),
            owners => bail!(
                "请先退出正在运行的原厂控制进程：{}",
                owners
                    .iter()
                    .map(|owner| owner.executable)
                    .collect::<Vec<_>>()
                    .join("、")
            ),
        }
    }

    fn open(pid: u32) -> Result<Self> {
        let handle = unsafe {
            OpenProcess(
                PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
                0,
                pid,
            )
        };
        if handle.is_null() {
            return Err(std::io::Error::last_os_error().into());
        }
        let handle = unsafe { OwnedHandle::from_raw_handle(handle) };
        ensure!(
            session_id(pid)? == session_id(std::process::id())?,
            "FnKey 位于其他 Windows 会话"
        );
        verify_image(handle.as_raw_handle())?;
        let identity = FnKeyIdentity {
            pid,
            creation_time: creation_time(handle.as_raw_handle())?,
        };
        Ok(Self {
            handle,
            identity,
            window: find_main_window(pid)?,
        })
    }

    pub(crate) fn identity(&self) -> FnKeyIdentity {
        self.identity
    }

    pub(crate) fn close(self, approved: FnKeyIdentity) -> Result<()> {
        ensure!(
            approved == self.identity,
            "原厂 FnKey 已重新启动，请重新选择模式并确认"
        );
        if self.wait_for_exit(0)? {
            return Ok(());
        }
        ensure!(
            matches!(owners()?.as_slice(), [owner] if owner.executable == FN_KEY && owner.pid == self.identity.pid),
            "原厂控制进程已变化，请重新选择模式"
        );
        ensure!(
            is_main_window(self.window, self.identity.pid),
            "原厂 FnKey 窗口已变化，请重试"
        );
        if unsafe { PostMessageW(self.window, WM_CLOSE, 0, 0) } == 0 {
            return Err(std::io::Error::last_os_error()).context("未能请求原厂 FnKey 退出");
        }
        ensure!(
            self.wait_for_exit(8000)?,
            "等待原厂 FnKey 退出超时，请从任务管理器退出后重试"
        );
        Ok(())
    }

    fn wait_for_exit(&self, milliseconds: u32) -> Result<bool> {
        match unsafe { WaitForSingleObject(self.handle.as_raw_handle(), milliseconds) } {
            WAIT_OBJECT_0 => Ok(true),
            WAIT_TIMEOUT => Ok(false),
            _ => Err(std::io::Error::last_os_error()).context("未能确认原厂 FnKey 已退出"),
        }
    }
}

fn session_id(pid: u32) -> Result<u32> {
    let mut session = 0;
    if unsafe { ProcessIdToSessionId(pid, &mut session) } == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(session)
}

fn creation_time(handle: HANDLE) -> Result<[u32; 2]> {
    let (mut created, mut exited, mut kernel, mut user) =
        <(FILETIME, FILETIME, FILETIME, FILETIME)>::default();
    if unsafe { GetProcessTimes(handle, &mut created, &mut exited, &mut kernel, &mut user) } == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok([created.dwLowDateTime, created.dwHighDateTime])
}

fn verify_image(handle: HANDLE) -> Result<()> {
    let app =
        package_string(|length, text| unsafe { GetApplicationUserModelId(handle, length, text) })?;
    ensure!(
        app.eq_ignore_ascii_case(APP_ID),
        "FnKey 不属于已核对的 CLEVO 应用包"
    );
    let name = package_string(|length, text| unsafe { GetPackageFullName(handle, length, text) })?;
    let name = name.encode_utf16().chain([0]).collect::<Vec<_>>();
    let directory = package_string(|length, text| unsafe {
        GetPackagePathByFullName(name.as_ptr(), length, text)
    })?;
    let expected = Path::new(&directory).join("FnKey").join(FN_KEY);
    let mut image = vec![0_u16; 32768];
    let mut length = image.len() as u32;
    if unsafe {
        QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, image.as_mut_ptr(), &mut length)
    } == 0
    {
        return Err(std::io::Error::last_os_error().into());
    }
    let image = String::from_utf16(&image[..length as usize])?;
    ensure!(
        image.eq_ignore_ascii_case(&expected.to_string_lossy()),
        "FnKey 映像与应用包不匹配"
    );
    Ok(())
}

fn package_string(read: impl Fn(*mut u32, *mut u16) -> u32) -> Result<String> {
    let mut length = 0;
    let code = read(&mut length, null_mut());
    if code != ERROR_INSUFFICIENT_BUFFER {
        return Err(std::io::Error::from_raw_os_error(code as i32).into());
    }
    let mut text = vec![0_u16; length as usize];
    let code = read(&mut length, text.as_mut_ptr());
    if code != 0 {
        return Err(std::io::Error::from_raw_os_error(code as i32).into());
    }
    Ok(String::from_utf16(
        text.split(|character| *character == 0)
            .next()
            .unwrap_or_default(),
    )?)
}

struct WindowSearch {
    pid: u32,
    window: HWND,
    multiple: bool,
}

fn find_main_window(pid: u32) -> Result<HWND> {
    let mut search = WindowSearch {
        pid,
        window: null_mut(),
        multiple: false,
    };
    let ok = unsafe {
        EnumWindows(
            Some(collect_window),
            (&mut search as *mut WindowSearch) as LPARAM,
        )
    };
    if ok == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    ensure!(
        !search.window.is_null() && !search.multiple,
        "未找到唯一的原厂 FnKey 主窗口"
    );
    Ok(search.window)
}

unsafe extern "system" fn collect_window(window: HWND, data: LPARAM) -> BOOL {
    let search = unsafe { &mut *(data as *mut WindowSearch) };
    if is_main_window(window, search.pid) {
        search.multiple |= !search.window.is_null();
        search.window = window;
    }
    1
}

fn is_main_window(window: HWND, pid: u32) -> bool {
    let mut owner = 0;
    if unsafe { GetWindowThreadProcessId(window, &mut owner) } == 0 || owner != pid {
        return false;
    }
    let mut title = [0_u16; 16];
    let length = unsafe { GetWindowTextW(window, title.as_mut_ptr(), title.len() as i32) };
    if !"FnKey"
        .encode_utf16()
        .eq(title[..length as usize].iter().copied())
    {
        return false;
    }
    let mut class = [0_u16; 128];
    let length = unsafe { GetClassNameW(window, class.as_mut_ptr(), class.len() as i32) };
    String::from_utf16_lossy(&class[..length as usize]).starts_with("WindowsForms10.Window.")
}
