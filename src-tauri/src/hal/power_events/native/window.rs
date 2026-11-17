use super::*;
use windows_sys::core::{GUID, w};

pub(super) struct Window {
    hwnd: HWND,
    module: HMODULE,
    class_name: Vec<u16>,
    power: Vec<HPOWERNOTIFY>,
    suspend: HPOWERNOTIFY,
}
impl Window {
    pub fn create(display: bool) -> Result<Self> {
        let module = unsafe { GetModuleHandleW(std::ptr::null()) };
        if module.is_null() {
            return Err(std::io::Error::last_os_error()).context("读取电源通知模块失败");
        }
        let class_name = super::super::window_class_name()?;
        let class = WNDCLASSW {
            lpfnWndProc: Some(callback),
            hInstance: module,
            lpszClassName: class_name.as_ptr(),
            ..Default::default()
        };
        if unsafe { RegisterClassW(&class) } == 0 {
            return Err(std::io::Error::last_os_error()).context("注册电源通知窗口类失败");
        }
        let mut window = Self {
            hwnd: std::ptr::null_mut(),
            module,
            class_name,
            power: Vec::new(),
            suspend: 0,
        };
        window.hwnd = unsafe {
            CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                window.class_name.as_ptr(),
                w!(""),
                WS_OVERLAPPED,
                0,
                0,
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                module,
                std::ptr::null(),
            )
        };
        if window.hwnd.is_null() {
            return Err(std::io::Error::last_os_error()).context("创建电源通知窗口失败");
        }
        window.register(SUPPLY)?;
        if display {
            window.register(DISPLAY)?;
        }
        window.suspend =
            unsafe { RegisterSuspendResumeNotification(window.hwnd, DEVICE_NOTIFY_WINDOW_HANDLE) };
        if window.suspend == 0 {
            return Err(std::io::Error::last_os_error()).context("注册挂起及恢复通知失败");
        }
        Ok(window)
    }
    fn register(&mut self, id: u128) -> Result<()> {
        let handle = unsafe {
            RegisterPowerSettingNotification(
                self.hwnd,
                &GUID::from_u128(id),
                DEVICE_NOTIFY_WINDOW_HANDLE,
            )
        };
        if handle == 0 {
            return Err(std::io::Error::last_os_error()).context("注册电源状态通知失败");
        }
        self.power.push(handle);
        Ok(())
    }
}
impl Drop for Window {
    fn drop(&mut self) {
        for handle in self.power.drain(..) {
            if unsafe { UnregisterPowerSettingNotification(handle) } == 0 {
                log::error!(
                    "[power-events] 注销供电通知失败：{}",
                    std::io::Error::last_os_error()
                );
            }
        }
        if self.suspend != 0 && unsafe { UnregisterSuspendResumeNotification(self.suspend) } == 0 {
            log::error!(
                "[power-events] 注销恢复通知失败：{}",
                std::io::Error::last_os_error()
            );
        }
        if !self.hwnd.is_null() && unsafe { DestroyWindow(self.hwnd) } == 0 {
            log::error!(
                "[power-events] 销毁通知窗口失败：{}",
                std::io::Error::last_os_error()
            );
        }
        if unsafe { UnregisterClassW(self.class_name.as_ptr(), self.module) } == 0 {
            log::error!(
                "[power-events] 注销通知窗口类失败：{}",
                std::io::Error::last_os_error()
            );
        }
    }
}
