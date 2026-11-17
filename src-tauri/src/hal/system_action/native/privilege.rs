use anyhow::{Context, Result, bail};
use windows_sys::Win32::{
    Foundation::{CloseHandle, ERROR_SUCCESS, GetLastError, HANDLE, LUID, SetLastError},
    Security::{
        AdjustTokenPrivileges, LUID_AND_ATTRIBUTES, LookupPrivilegeValueW, SE_PRIVILEGE_ENABLED,
        TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY,
    },
    System::Threading::{GetCurrentProcess, OpenProcessToken},
};

pub(super) struct ShutdownPrivilege {
    token: HANDLE,
    previous: Option<TOKEN_PRIVILEGES>,
}
impl ShutdownPrivilege {
    pub(super) fn enable() -> Result<Self> {
        let mut token = std::ptr::null_mut();
        if unsafe {
            OpenProcessToken(
                GetCurrentProcess(),
                TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY,
                &mut token,
            )
        } == 0
        {
            return Err(std::io::Error::last_os_error()).context("读取系统动作令牌失败");
        }
        let mut guard = Self {
            token,
            previous: None,
        };
        let name: Vec<u16> = "SeShutdownPrivilege\0".encode_utf16().collect();
        let mut luid = LUID::default();
        if unsafe { LookupPrivilegeValueW(std::ptr::null(), name.as_ptr(), &mut luid) } == 0 {
            return Err(std::io::Error::last_os_error()).context("读取关机权限标识失败");
        }
        let wanted = TOKEN_PRIVILEGES {
            PrivilegeCount: 1,
            Privileges: [LUID_AND_ATTRIBUTES {
                Luid: luid,
                Attributes: SE_PRIVILEGE_ENABLED,
            }],
        };
        let mut previous = TOKEN_PRIVILEGES::default();
        let mut returned = 0;
        unsafe { SetLastError(ERROR_SUCCESS) };
        let changed = unsafe {
            AdjustTokenPrivileges(
                token,
                0,
                &wanted,
                std::mem::size_of::<TOKEN_PRIVILEGES>() as u32,
                &mut previous,
                &mut returned,
            )
        };
        let error = unsafe { GetLastError() };
        if changed == 0 || error != ERROR_SUCCESS {
            bail!("当前令牌无法启用系统电源动作权限（{error}）");
        }
        guard.previous = Some(previous);
        Ok(guard)
    }
    pub(super) fn restore(&mut self) -> Result<()> {
        let Some(previous) = &self.previous else {
            return Ok(());
        };
        unsafe { SetLastError(ERROR_SUCCESS) };
        let restored = unsafe {
            AdjustTokenPrivileges(
                self.token,
                0,
                previous,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        if restored == 0 || unsafe { GetLastError() } != ERROR_SUCCESS {
            return Err(std::io::Error::last_os_error()).context("恢复系统动作令牌失败");
        }
        self.previous = None;
        Ok(())
    }
}
impl Drop for ShutdownPrivilege {
    fn drop(&mut self) {
        if let Err(error) = self.restore() {
            log::error!("[system-action] {error:#}");
        }
        unsafe { CloseHandle(self.token) };
    }
}
