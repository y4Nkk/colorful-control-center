//! 低级键盘与输入活动事件唯一实现；业务拥有订阅及启停，回调不执行硬件写入。
use anyhow::{Result, bail};
use ccc_core::panel_hotkey::Activity;
use std::{
    cell::RefCell,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU32, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::*,
    System::{LibraryLoader::GetModuleHandleW, Threading::GetCurrentThreadId},
    UI::WindowsAndMessaging::*,
};
#[derive(Clone, Copy, Debug)]
pub struct KeyEvent {
    pub scan: u16,
    pub down: bool,
    pub injected: bool,
    pub at: Instant,
}
#[derive(Clone, Default)]
pub struct Stopper {
    stop: Arc<AtomicBool>,
    thread: Arc<AtomicU32>,
}
impl Stopper {
    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
        let id = self.thread.load(Ordering::SeqCst);
        if id != 0 {
            unsafe {
                PostThreadMessageW(id, WM_QUIT, 0, 0);
            }
        }
    }
}
enum Mode {
    Keys(Box<dyn Fn(KeyEvent) -> bool + Send + Sync>),
    Activity(Box<dyn Fn(Activity) + Send + Sync>),
}
struct Control {
    stopper: Stopper,
    error: Mutex<Option<String>>,
    mode: Mode,
    failed: Box<dyn Fn(String) + Send + Sync>,
}
impl Control {
    fn fail(&self, error: String) {
        if let Ok(mut value) = self.error.lock() {
            *value = Some(error.clone());
        }
        self.stopper.stop();
        (self.failed)(error);
    }
    fn dispatch(&self, key: KeyEvent) -> bool {
        if self.stopper.stop.load(Ordering::SeqCst) {
            return false;
        }
        let Mode::Keys(callback) = &self.mode else {
            return false;
        };
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| callback(key))).unwrap_or_else(
            |_| {
                self.fail("键盘事件处理异常，已停止订阅".into());
                false
            },
        )
    }
    fn activity(&self, message: u32) {
        if self.stopper.stop.load(Ordering::SeqCst) {
            return;
        }
        let Mode::Activity(callback) = &self.mode else {
            return;
        };
        let activity = match message {
            WM_KEYDOWN | WM_SYSKEYDOWN => Activity::KeyDown,
            WM_KEYUP | WM_SYSKEYUP => return,
            _ => Activity::Pointer,
        };
        if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| callback(activity))).is_err() {
            self.fail("输入活动处理异常，已停止订阅".into());
        }
    }
}
thread_local! {static CONTEXT:RefCell<Option<Arc<Control>>>=const {RefCell::new(None)};}
pub struct Session {
    control: Arc<Control>,
    thread: Option<JoinHandle<()>>,
}
impl Session {
    pub fn start(
        key: impl Fn(KeyEvent) -> bool + Send + Sync + 'static,
        failed: impl Fn(String) + Send + Sync + 'static,
    ) -> Result<Self> {
        Self::prepare(Mode::Keys(Box::new(key)), failed)
    }
    pub fn input_activity(
        activity: impl Fn(Activity) + Send + Sync + 'static,
        failed: impl Fn(String) + Send + Sync + 'static,
    ) -> Result<Self> {
        Self::prepare(Mode::Activity(Box::new(activity)), failed)
    }
    fn prepare(mode: Mode, failed: impl Fn(String) + Send + Sync + 'static) -> Result<Self> {
        let control = Arc::new(Control {
            stopper: Stopper::default(),
            error: Mutex::new(None),
            mode,
            failed: Box::new(failed),
        });
        let (tx, rx) = mpsc::sync_channel(1);
        let shared = control.clone();
        let thread = thread::Builder::new()
            .name("input-events".into())
            .spawn(move || run(shared, tx))?;
        let mut session = Self {
            control,
            thread: Some(thread),
        };
        match rx.recv_timeout(Duration::from_secs(3)) {
            Ok(Ok(())) => Ok(session),
            value => {
                let error = match value {
                    Ok(Err(error)) => error,
                    Err(error) => format!("键盘事件准备失败：{error}"),
                    Ok(Ok(())) => unreachable!(),
                };
                session.stop();
                bail!(error)
            }
        }
    }
    pub fn stopper(&self) -> Stopper {
        self.control.stopper.clone()
    }
    pub fn error(&self) -> Option<String> {
        self.control.error.lock().ok().and_then(|e| e.clone())
    }
    fn stop(&mut self) {
        self.control.stopper.stop();
        if let Some(thread) = self.thread.take() {
            if thread.thread().id() != thread::current().id() {
                let _ = thread.join();
            }
        }
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        self.stop();
    }
}
struct Hooks(Vec<HHOOK>);
impl Hooks {
    fn install(mode: &Mode) -> Result<Self> {
        let mut hooks = Self(Vec::new());
        let count = if matches!(mode, Mode::Activity(_)) {
            2
        } else {
            1
        };
        for kind in [WH_KEYBOARD_LL, WH_MOUSE_LL].into_iter().take(count) {
            let hook = unsafe {
                SetWindowsHookExW(kind, Some(callback), GetModuleHandleW(std::ptr::null()), 0)
            };
            if hook.is_null() {
                bail!("无法启动输入事件：{}", std::io::Error::last_os_error());
            }
            hooks.0.push(hook);
        }
        Ok(hooks)
    }
}
impl Drop for Hooks {
    fn drop(&mut self) {
        for hook in self.0.drain(..) {
            if unsafe { UnhookWindowsHookEx(hook) } == 0 {
                log::warn!(
                    "[input-events] 解绑输入钩子失败：{}",
                    std::io::Error::last_os_error()
                );
            }
        }
    }
}
fn run(control: Arc<Control>, ready: mpsc::SyncSender<std::result::Result<(), String>>) {
    let mut message = MSG::default();
    unsafe {
        PeekMessageW(&mut message, std::ptr::null_mut(), 0, 0, PM_NOREMOVE);
    }
    control
        .stopper
        .thread
        .store(unsafe { GetCurrentThreadId() }, Ordering::SeqCst);
    if control.stopper.stop.load(Ordering::SeqCst) {
        control.stopper.thread.store(0, Ordering::SeqCst);
        return;
    }
    CONTEXT.with(|ctx| *ctx.borrow_mut() = Some(control.clone()));
    let hooks = Hooks::install(&control.mode);
    if let Err(error) = hooks.as_ref() {
        let error = format!("{error:#}");
        let _ = ready.send(Err(error.clone()));
        control.fail(error);
    } else {
        let _ = ready.send(Ok(()));
        while !control.stopper.stop.load(Ordering::SeqCst) {
            let status = unsafe { GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) };
            if status <= 0 {
                if !control.stopper.stop.load(Ordering::SeqCst) {
                    control.fail(format!("键盘事件循环已退出 ({status})"));
                }
                break;
            }
            unsafe {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }
    drop(hooks);
    control.stopper.thread.store(0, Ordering::SeqCst);
    CONTEXT.with(|ctx| *ctx.borrow_mut() = None);
}
fn decode(virtual_key: u32, scan: u32, flags: u32, message: u32) -> Option<KeyEvent> {
    if scan == 0
        || scan > 255
        || ![WM_KEYDOWN, WM_SYSKEYDOWN, WM_KEYUP, WM_SYSKEYUP].contains(&message)
    {
        return None;
    }
    Some(KeyEvent {
        scan: if virtual_key == 19 {
            ccc_core::keymap::PAUSE_SCAN_CODE
        } else {
            scan as u16
                | if (virtual_key == 255 || flags & LLKHF_EXTENDED != 0)
                    && ![144, 161].contains(&virtual_key)
                {
                    0xe000
                } else {
                    0
                }
        },
        down: [WM_KEYDOWN, WM_SYSKEYDOWN].contains(&message),
        injected: flags & LLKHF_INJECTED != 0,
        at: Instant::now(),
    })
}
unsafe extern "system" fn callback(code: i32, message: WPARAM, value: LPARAM) -> LRESULT {
    if code >= 0 {
        let consume = CONTEXT.with(|ctx| {
            let context = ctx.borrow();
            let Some(control) = context.as_ref() else {
                return false;
            };
            match &control.mode {
                Mode::Keys(_) if code == HC_ACTION as i32 => {
                    let raw = unsafe { &*(value as *const KBDLLHOOKSTRUCT) };
                    decode(raw.vkCode, raw.scanCode, raw.flags, message as u32)
                        .is_some_and(|key| control.dispatch(key))
                }
                Mode::Keys(_) => false,
                Mode::Activity(_) => {
                    control.activity(message as u32);
                    false
                }
            }
        });
        if consume {
            return 1;
        }
    }
    unsafe { CallNextHookEx(std::ptr::null_mut(), code, message, value) }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn input_activity_keeps_only_categories_ignores_key_release_and_stops_without_consuming_keys() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let seen = events.clone();
        let control = Control {
            stopper: Stopper::default(),
            error: Mutex::new(None),
            mode: Mode::Activity(Box::new(move |event| seen.lock().unwrap().push(event))),
            failed: Box::new(|_| {}),
        };
        for message in [
            WM_KEYUP,
            WM_SYSKEYUP,
            WM_KEYDOWN,
            WM_SYSKEYDOWN,
            WM_MOUSEMOVE,
            WM_MOUSEWHEEL,
        ] {
            control.activity(message);
        }
        assert_eq!(
            *events.lock().unwrap(),
            [
                Activity::KeyDown,
                Activity::KeyDown,
                Activity::Pointer,
                Activity::Pointer
            ]
        );
        assert!(!control.dispatch(decode(0, 1, 0, WM_KEYDOWN).unwrap()));
        control.stopper.stop();
        control.activity(WM_MOUSEMOVE);
        assert_eq!(events.lock().unwrap().len(), 4);
    }
    #[test]
    fn original_extended_scan_and_injected_flag_are_separate_from_virtual_keys() {
        let key = decode(0, 29, LLKHF_EXTENDED | LLKHF_INJECTED, WM_KEYDOWN).unwrap();
        assert_eq!(key.scan, 0xe01d);
        assert!(key.down && key.injected);
        assert!(!decode(0, 29, 0, WM_KEYUP).unwrap().down);
        assert!(decode(0, 256, 0, WM_KEYDOWN).is_none());
        assert!(decode(0, 0, 0, WM_KEYDOWN).is_none());
    }
    #[test]
    fn oem_numlock_right_shift_fn_and_pause_use_the_native_filter_scan_codes() {
        assert_eq!(
            decode(144, 69, LLKHF_EXTENDED, WM_KEYDOWN).unwrap().scan,
            69
        );
        assert_eq!(
            decode(161, 54, LLKHF_EXTENDED, WM_KEYDOWN).unwrap().scan,
            54
        );
        assert_eq!(decode(255, 119, 0, WM_KEYDOWN).unwrap().scan, 0xe077);
        for message in [WM_KEYDOWN, WM_KEYUP] {
            assert_eq!(
                decode(19, 69, 0, message).unwrap().scan,
                ccc_core::keymap::PAUSE_SCAN_CODE
            );
        }
    }
    #[test]
    fn stopped_subscription_cannot_consume_or_forward_further_events() {
        let calls = Arc::new(AtomicU32::new(0));
        let seen = calls.clone();
        let control = Control {
            stopper: Stopper::default(),
            error: Mutex::new(None),
            mode: Mode::Keys(Box::new(move |_| {
                seen.fetch_add(1, Ordering::SeqCst);
                true
            })),
            failed: Box::new(|_| {}),
        };
        let event = decode(0, 1, 0, WM_KEYDOWN).unwrap();
        assert!(control.dispatch(event));
        control.stopper.stop();
        assert!(!control.dispatch(event));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
}
