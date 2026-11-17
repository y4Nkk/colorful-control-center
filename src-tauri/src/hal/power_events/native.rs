use super::*;
use anyhow::Context;
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
    System::{LibraryLoader::GetModuleHandleW, Power::*, Threading::GetCurrentThreadId},
    UI::WindowsAndMessaging::*,
};

mod window;
type Sink = Arc<dyn Fn(Event) -> Result<()> + Send + Sync>;
thread_local! { static CONTEXT: RefCell<Option<Arc<Control>>> = const { RefCell::new(None) }; }

struct Control {
    stopping: AtomicBool,
    thread: AtomicU32,
    error: Mutex<Option<String>>,
    sink: Sink,
    failed: Arc<dyn Fn(String) + Send + Sync>,
}
impl Control {
    fn stop(&self) {
        self.stopping.store(true, Ordering::SeqCst);
        let id = self.thread.load(Ordering::SeqCst);
        if id != 0 {
            unsafe {
                PostThreadMessageW(id, WM_QUIT, 0, 0);
            }
        }
    }
    fn fail(&self, error: String) {
        let first = !self.stopping.swap(true, Ordering::SeqCst);
        if let Ok(mut current) = self.error.lock() {
            *current = Some(error.clone());
        }
        self.stop();
        if first {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (self.failed)(error)));
        }
    }
    fn dispatch(&self, event: Event) {
        if self.stopping.load(Ordering::SeqCst) {
            return;
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (self.sink)(event)));
        match result {
            Ok(Ok(())) => (),
            Ok(Err(error)) => self.fail(format!("电源事件处理失败：{error:#}")),
            Err(_) => self.fail("电源事件处理异常，监听已停止".into()),
        }
    }
}

#[derive(Clone)]
pub(crate) struct Stopper(Arc<Control>);
impl Stopper {
    pub fn stop(&self) {
        self.0.stop();
    }
}

pub(crate) struct Session {
    control: Arc<Control>,
    thread: Option<JoinHandle<()>>,
    route: Option<Route>,
}
impl Session {
    pub fn start(
        policy: DisplayPolicy,
        sink: impl Fn(Event) -> Result<()> + Send + Sync + 'static,
        failed: impl Fn(String) + Send + Sync + 'static,
    ) -> Result<Self> {
        let control = Arc::new(Control {
            stopping: AtomicBool::new(false),
            thread: AtomicU32::new(0),
            error: Mutex::new(None),
            sink: Arc::new(sink),
            failed: Arc::new(failed),
        });
        let (tx, rx) = mpsc::sync_channel(1);
        let shared = control.clone();
        let thread = thread::Builder::new()
            .name("power-events".into())
            .spawn(move || run(shared, policy, tx))?;
        let mut session = Self {
            control,
            thread: Some(thread),
            route: None,
        };
        match rx.recv_timeout(Duration::from_secs(3)) {
            Ok(Ok(route)) => session.route = Some(route),
            Ok(Err(error)) => session.control.fail(error),
            Err(error) => session.control.fail(format!("电源通知准备失败：{error}")),
        }
        Ok(session)
    }
    pub fn stopper(&self) -> Stopper {
        Stopper(self.control.clone())
    }
    pub fn status(&self) -> Result<Route> {
        if let Some(error) = self
            .control
            .error
            .lock()
            .map_err(|_| anyhow::anyhow!("电源通知状态异常"))?
            .as_ref()
        {
            bail!("{error}");
        }
        if self
            .thread
            .as_ref()
            .is_none_or(|thread| thread.is_finished())
        {
            bail!("电源通知线程已退出");
        }
        self.route.clone().context("电源通知尚未就绪")
    }
    pub fn stop(&mut self) -> Result<()> {
        self.control.stop();
        let deadline = Instant::now() + Duration::from_secs(2);
        while self
            .thread
            .as_ref()
            .is_some_and(|thread| !thread.is_finished())
            && Instant::now() < deadline
        {
            thread::sleep(Duration::from_millis(5));
        }
        if self
            .thread
            .as_ref()
            .is_some_and(|thread| !thread.is_finished())
        {
            bail!("电源通知仍在退出，未启动替代监听");
        }
        if let Some(thread) = self.thread.take() {
            thread
                .join()
                .map_err(|_| anyhow::anyhow!("电源通知线程退出异常"))?;
        }
        Ok(())
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        if let Err(error) = self.stop() {
            log::error!("[power-events] {error:#}");
        }
    }
}

fn run(
    control: Arc<Control>,
    policy: DisplayPolicy,
    ready: mpsc::SyncSender<std::result::Result<Route, String>>,
) {
    let mut message = MSG::default();
    unsafe {
        PeekMessageW(&mut message, std::ptr::null_mut(), 0, 0, PM_NOREMOVE);
    }
    control
        .thread
        .store(unsafe { GetCurrentThreadId() }, Ordering::SeqCst);
    CONTEXT.with(|ctx| *ctx.borrow_mut() = Some(control.clone()));
    if !control.stopping.load(Ordering::SeqCst) {
        match prepare(policy) {
            Ok((window, route)) => {
                let _ = ready.send(Ok(route));
                pump(&control, &mut message);
                drop(window);
            }
            Err(error) => {
                let error = format!("{error:#}");
                control.fail(error.clone());
                let _ = ready.send(Err(error));
            }
        }
    }
    CONTEXT.with(|ctx| *ctx.borrow_mut() = None);
    control.thread.store(0, Ordering::SeqCst);
}

fn prepare(policy: DisplayPolicy) -> Result<(window::Window, Route)> {
    let mut caps = SYSTEM_POWER_CAPABILITIES::default();
    let modern = unsafe { GetPwrCapabilities(&mut caps) }.then_some(caps.AoAc);
    let route = route(modern, policy);
    let window = window::Window::create(route.display_events)?;
    // 注册通知的首个值建立基线，避免查询结果与排队的初始通知产生虚假变化。
    Ok((window, route))
}

fn pump(control: &Control, message: &mut MSG) {
    while !control.stopping.load(Ordering::SeqCst) {
        let status = unsafe { GetMessageW(message, std::ptr::null_mut(), 0, 0) };
        if status <= 0 {
            if !control.stopping.load(Ordering::SeqCst) {
                control.fail(format!(
                    "电源通知循环已退出 ({status})：{}",
                    std::io::Error::last_os_error()
                ));
            }
            break;
        }
        unsafe {
            TranslateMessage(message);
            DispatchMessageW(message);
        }
    }
}

unsafe extern "system" fn callback(hwnd: HWND, message: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if message == WM_DESTROY {
        CONTEXT.with(|ctx| {
            if let Some(control) = ctx.borrow().as_ref() {
                if !control.stopping.load(Ordering::SeqCst) {
                    control.fail("电源通知窗口意外关闭".into());
                }
            }
        });
    }
    if message != WM_POWERBROADCAST {
        return unsafe { DefWindowProcW(hwnd, message, wp, lp) };
    }
    CONTEXT.with(|ctx| {
        if let Some(control) = ctx.borrow().as_ref() {
            match unsafe { event(wp, lp) } {
                Ok(Some(event)) => control.dispatch(event),
                Err(error) => control.fail(format!("电源通知无效：{error:#}")),
                _ => (),
            }
        }
    });
    1
}

unsafe fn event(wp: WPARAM, lp: LPARAM) -> Result<Option<Event>> {
    match wp as u32 {
        PBT_APMSUSPEND => Ok(Some(Event::Suspend)),
        PBT_APMRESUMEAUTOMATIC | PBT_APMRESUMESUSPEND => Ok(Some(Event::Resume)),
        PBT_POWERSETTINGCHANGE if lp != 0 => {
            let setting = unsafe { &*(lp as *const POWERBROADCAST_SETTING) };
            let guid = setting.PowerSetting;
            let id = ((guid.data1 as u128) << 96)
                | ((guid.data2 as u128) << 80)
                | ((guid.data3 as u128) << 64)
                | u64::from_be_bytes(guid.data4) as u128;
            if ![SUPPLY, DISPLAY].contains(&id) {
                return Ok(None);
            }
            if setting.DataLength != 4 {
                bail!("电源通知长度无效");
            }
            decode(id, unsafe {
                std::slice::from_raw_parts(setting.Data.as_ptr(), 4)
            })
        }
        _ => Ok(None),
    }
}
