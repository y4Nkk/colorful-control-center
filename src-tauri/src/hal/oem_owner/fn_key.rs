use std::{
    os::windows::io::{AsRawHandle, OwnedHandle},
    ptr::null_mut,
};

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use windows_sys::{
    Win32::{
        Foundation::{FILETIME, HANDLE, HWND, LPARAM},
        System::Threading::GetProcessTimes,
        UI::WindowsAndMessaging::{
            EnumWindows, GetClassNameW, GetWindowTextW, GetWindowThreadProcessId, PostMessageW,
            WM_CLOSE,
        },
    },
    core::BOOL,
};

use super::{
    FN_KEY, control_owners,
    process::{FN_KEY_TARGET, open_verified, wait_for_exit},
};

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
        let active = control_owners()?;
        match active.as_slice() {
            [] => Ok(None),
            [owner] if owner.executable == FN_KEY => Self::open(owner.pid)
                .map(Some)
                .context("无法核验原厂 FnKey 的关闭入口，请从任务管理器退出 FnKey.exe 后重试")
                .context(owner.blocked()),
            [owner, ..] => Err(owner.blocked().context(format!(
                "请先退出正在运行的原厂控制进程：{}",
                active
                    .iter()
                    .map(|owner| owner.executable)
                    .collect::<Vec<_>>()
                    .join("、")
            ))),
        }
    }

    fn open(pid: u32) -> Result<Self> {
        let handle = open_verified(pid, &FN_KEY_TARGET, 0)?;
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
            "原厂 FnKey 已重新启动，请重新核对后确认"
        );
        if wait_for_exit(&self.handle, 0)? {
            return Ok(());
        }
        ensure!(
            matches!(control_owners()?.as_slice(), [owner] if owner.executable == FN_KEY && owner.pid == self.identity.pid),
            "原厂控制进程已变化，请重新核对"
        );
        ensure!(
            is_main_window(self.window, self.identity.pid),
            "原厂 FnKey 窗口已变化，请重试"
        );
        if unsafe { PostMessageW(self.window, WM_CLOSE, 0, 0) } == 0 {
            return Err(std::io::Error::last_os_error()).context("未能请求原厂 FnKey 退出");
        }
        ensure!(
            wait_for_exit(&self.handle, 8000)?,
            "等待原厂 FnKey 退出超时，请从任务管理器退出后重试"
        );
        Ok(())
    }
}

fn creation_time(handle: HANDLE) -> Result<[u32; 2]> {
    let (mut created, mut exited, mut kernel, mut user) =
        <(FILETIME, FILETIME, FILETIME, FILETIME)>::default();
    if unsafe { GetProcessTimes(handle, &mut created, &mut exited, &mut kernel, &mut user) } == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok([created.dwLowDateTime, created.dwHighDateTime])
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
