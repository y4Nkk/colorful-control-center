mod integrity;

use crate::hal::authorization::session::events::Event;
use anyhow::{Context, Result, anyhow};
use std::{
    io,
    mem::size_of,
    ptr::{null, null_mut},
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
};
use tauri::{AppHandle, Manager};
use windows_sys::Win32::{
    Foundation::{ERROR_ALREADY_EXISTS, GetLastError, LocalFree, SetLastError, WAIT_OBJECT_0},
    Security::{
        Authorization::{ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1},
        SECURITY_ATTRIBUTES,
    },
    System::Threading::{
        CreateEventExW, EVENT_MODIFY_STATE, INFINITE, OpenEventW, SYNCHRONIZATION_SYNCHRONIZE,
        WaitForMultipleObjects,
    },
    UI::WindowsAndMessaging::{AllowSetForegroundWindow, FindWindowW, GetWindowThreadProcessId},
};

const SIGNAL_ACCESS: u32 = SYNCHRONIZATION_SYNCHRONIZE | EVENT_MODIFY_STATE;

pub(crate) struct Activation {
    pub window_class: String,
    signal: Event,
}

impl Activation {
    pub(super) fn claim(user: &str, show: bool) -> Result<Option<Self>> {
        Self::claim_named(window_class(user), user, show)
    }

    fn claim_named(window_class: String, user: &str, show: bool) -> Result<Option<Self>> {
        let (signal, existing) = create_signal(&window_class, user)?;
        if existing {
            if show {
                grant_foreground(&window_class);
                signal.signal().context("唤回现有主窗口失败")?;
            }
            return Ok(None);
        }
        Ok(Some(Self {
            window_class,
            signal,
        }))
    }

    pub(super) fn retain(user: &str) -> Result<Self> {
        Self::retain_named(window_class(user))
    }

    fn retain_named(window_class: String) -> Result<Self> {
        let name = event_name(&window_class);
        let signal = Event::owned(unsafe { OpenEventW(SIGNAL_ACCESS, 0, name.as_ptr()) })
            .context("打开原应用的唤回事件失败")?;
        Ok(Self {
            window_class,
            signal,
        })
    }

    pub(crate) fn listen(self, app: &AppHandle) -> Result<()> {
        let signal = Arc::new(self.signal);
        let stop = Arc::new(Event::anonymous()?);
        let handle = app.clone();
        let worker = {
            let signal = signal.clone();
            let stop = stop.clone();
            thread::Builder::new()
                .name("window-activation".into())
                .spawn(move || listen(&signal, &stop, &handle))
                .context("启动主窗口唤回监听失败")?
        };
        app.manage(Listener {
            _signal: signal,
            stop,
            worker: Mutex::new(Some(worker)),
        });
        Ok(())
    }
}

struct Listener {
    _signal: Arc<Event>,
    stop: Arc<Event>,
    worker: Mutex<Option<JoinHandle<Result<()>>>>,
}

pub(crate) fn stop_activation(app: &AppHandle) -> Result<()> {
    if let Some(listener) = app.try_state::<Listener>() {
        listener.stop.signal()?;
        let worker = listener
            .worker
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take();
        if let Some(worker) = worker {
            worker.join().map_err(|_| anyhow!("主窗口唤回线程异常"))??;
        }
    }
    Ok(())
}

fn listen(signal: &Arc<Event>, stop: &Event, app: &AppHandle) -> Result<()> {
    let handles = [stop.raw(), signal.raw()];
    loop {
        match unsafe { WaitForMultipleObjects(2, handles.as_ptr(), 0, INFINITE) } {
            WAIT_OBJECT_0 => return Ok(()),
            status if status == WAIT_OBJECT_0 + 1 => {
                if crate::shutdown::closing() {
                    return signal.signal().context("保留重启期间的窗口唤回信号");
                }
                let handle = app.clone();
                let queued = signal.clone();
                if let Err(error) = app.run_on_main_thread(move || {
                    if crate::shutdown::closing() {
                        if let Err(error) = queued.signal() {
                            log::error!("[startup] 保留窗口唤回信号失败：{error:#}");
                        }
                    } else {
                        crate::tray::show_main(&handle);
                    }
                }) {
                    log::error!("[startup] 唤回主窗口失败：{error:#}");
                }
            }
            _ => return Err(io::Error::last_os_error()).context("等待主窗口唤回信号失败"),
        }
    }
}

fn window_class(user: &str) -> String {
    format!("{}.main.{user}", env!("CARGO_PKG_NAME"))
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain([0]).collect()
}

fn event_name(window_class: &str) -> Vec<u16> {
    wide(&format!("Local\\{window_class}"))
}

fn create_signal(window_class: &str, user: &str) -> Result<(Event, bool)> {
    let sddl = wide(&format!("D:P(A;;{SIGNAL_ACCESS:#010x};;;{user})"));
    let mut descriptor = null_mut();
    if unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            SDDL_REVISION_1,
            &mut descriptor,
            null_mut(),
        )
    } == 0
    {
        return Err(io::Error::last_os_error()).context("生成主窗口唤回事件的权限失败");
    }
    let attributes = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor,
        bInheritHandle: 0,
    };
    let name = event_name(window_class);
    // 管理员进程用同账户的中完整性令牌创建事件，让普通快捷方式可以打开该信号。
    let result = integrity::create_with_window_integrity(|| {
        let (raw, status) = unsafe {
            SetLastError(0);
            let raw = CreateEventExW(&attributes, name.as_ptr(), 0, SIGNAL_ACCESS);
            (raw, GetLastError())
        };
        if raw.is_null() {
            return Err(io::Error::from_raw_os_error(status as i32))
                .context("创建主窗口唤回事件失败");
        }
        Ok((Event::owned(raw)?, status == ERROR_ALREADY_EXISTS))
    });
    unsafe { LocalFree(descriptor) };
    result
}

fn grant_foreground(window_class: &str) {
    let hwnd = unsafe { FindWindowW(wide(window_class).as_ptr(), null()) };
    if !hwnd.is_null() {
        let mut pid = 0;
        unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
        if pid != 0 {
            unsafe { AllowSetForegroundWindow(pid) };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hal::authorization::{self, session::events::Binding};

    #[test]
    fn duplicate_launch_signals_once_and_duplicate_login_is_silent() {
        use windows_sys::Win32::System::Threading::EVENT_ALL_ACCESS;
        let user = authorization::user().unwrap();
        let class = Binding::new().unwrap().token;
        let primary = Activation::claim_named(class.clone(), &user, true)
            .unwrap()
            .unwrap();
        let full_access = unsafe { OpenEventW(EVENT_ALL_ACCESS, 0, event_name(&class).as_ptr()) };
        assert!(Event::owned(full_access).is_err());
        assert!(
            Activation::claim_named(class.clone(), &user, false)
                .unwrap()
                .is_none()
        );
        assert!(!primary.signal.signaled().unwrap());
        assert!(
            Activation::claim_named(class, &user, true)
                .unwrap()
                .is_none()
        );
        assert!(primary.signal.signaled().unwrap());
        assert!(!primary.signal.signaled().unwrap());
    }

    #[test]
    fn restart_reserves_the_endpoint_and_keeps_activation_queued() {
        let user = authorization::user().unwrap();
        let class = Binding::new().unwrap().token;
        let primary = Activation::claim_named(class.clone(), &user, false)
            .unwrap()
            .unwrap();
        let replacement = Activation::retain_named(class.clone()).unwrap();
        drop(primary);
        assert!(
            Activation::claim_named(class.clone(), &user, true)
                .unwrap()
                .is_none()
        );
        assert!(replacement.signal.signaled().unwrap());
        drop(replacement);
        assert!(
            Activation::claim_named(class, &user, false)
                .unwrap()
                .is_some()
        );
    }
}
