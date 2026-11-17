use super::*;
use std::sync::atomic::Ordering;
use std::{sync::mpsc, thread, time::Duration};
use tauri::{Emitter, PhysicalPosition, WebviewUrl, WebviewWindowBuilder};

pub(super) struct Timer {
    sender: mpsc::Sender<(u64, Duration)>,
    worker: Option<thread::JoinHandle<()>>,
    stop: mpsc::Sender<()>,
}
impl Drop for Timer {
    fn drop(&mut self) {
        let _ = self.stop.send(());
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
impl Timer {
    fn new(app: AppHandle) -> Result<Self> {
        let (sender, receiver) = mpsc::channel();
        let (stop, stopping) = mpsc::channel();
        let worker = thread::Builder::new()
            .name("desktop-osd".into())
            .spawn(move || {
                let mut pending = None;
                while stopping.try_recv().is_err() {
                    match receiver.recv_timeout(Duration::from_millis(40)) {
                        Ok(value) => pending = Some(value),
                        Err(mpsc::RecvTimeoutError::Timeout) => (),
                        Err(_) => break,
                    }
                    if let Some((sequence, deadline)) = pending {
                        if app.state::<Controller>().now() >= deadline {
                            let handle = app.clone();
                            if let Err(error) =
                                app.run_on_main_thread(move || hide(&handle, sequence))
                            {
                                report(&app, sequence, anyhow!(error));
                            }
                            pending = None;
                        }
                    }
                }
            })?;
        Ok(Self {
            sender,
            worker: Some(worker),
            stop,
        })
    }
}
fn emit(app: &AppHandle) {
    if let Ok(snapshot) = app.state::<Controller>().snapshot(app) {
        if let Err(error) = app.emit_to("osd", "osd-state", snapshot) {
            log::warn!("[osd] 通知失败：{error}");
        }
    }
}
fn report(app: &AppHandle, sequence: u64, error: anyhow::Error) {
    log::error!("[osd] {error:#}");
    if let Ok(mut state) = app.state::<Controller>().state.lock() {
        state.fail(sequence, format!("{error:#}"));
    }
    emit(app);
}
fn ensure_window(app: &AppHandle) -> Result<tauri::WebviewWindow> {
    if let Some(window) = app.get_webview_window("osd") {
        window.set_ignore_cursor_events(true)?;
        return Ok(window);
    }
    let mut builder =
        WebviewWindowBuilder::new(app, "osd", WebviewUrl::App("index.html?surface=osd".into()))
            .title("操作提示")
            .on_page_load(page_loaded)
            .inner_size(380.0, 120.0)
            .visible(false)
            .focused(false)
            .focusable(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .decorations(false)
            .transparent(true)
            .shadow(false)
            .resizable(false);
    if !app
        .state::<crate::preferences::Controller>()
        .runtime_acceleration()
    {
        builder = builder.additional_browser_args(
            "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --disable-gpu",
        );
    }
    let window = builder.build()?;
    window.set_ignore_cursor_events(true)?;
    Ok(window)
}
fn page_loaded(window: tauri::WebviewWindow, payload: tauri::webview::PageLoadPayload<'_>) {
    let app = window.app_handle();
    let ready = matches!(payload.event(), tauri::webview::PageLoadEvent::Finished);
    let controller = app.state::<Controller>();
    controller.ready.store(ready, Ordering::Release);
    if !ready {
        return;
    }
    let sequence = controller
        .state
        .lock()
        .ok()
        .and_then(|state| state.pending_sequence());
    if let Some(sequence) = sequence {
        let handle = app.clone();
        if let Err(error) = app.run_on_main_thread(move || present(&handle, sequence)) {
            report(app, sequence, anyhow!(error));
        }
    }
}
pub(super) fn present(app: &AppHandle, sequence: u64) {
    let result = (|| -> Result<()> {
        let controller = app.state::<Controller>();
        if controller
            .state
            .lock()
            .map_err(|_| anyhow!("屏显状态异常"))?
            .pending_sequence()
            != Some(sequence)
        {
            return Ok(());
        }
        let window = ensure_window(app)?;
        if !controller.ready.load(Ordering::Acquire) {
            return Ok(());
        }
        let preferred = app
            .get_webview_window("main")
            .and_then(|w| w.current_monitor().ok().flatten());
        let monitor = match preferred {
            Some(monitor) => monitor,
            None => window
                .primary_monitor()?
                .ok_or_else(|| anyhow!("没有可用的屏显显示器"))?,
        };
        let work = monitor.work_area();
        let scale = monitor.scale_factor();
        let zoom = controller
            .snapshot(app)?
            .appearance
            .map_or(1.0, |p| f64::from(p.interface_scale) / 100.0);
        let width = (380.0 * scale * zoom).min(f64::from(work.size.width));
        let height = (120.0 * scale * zoom).min(f64::from(work.size.height));
        window.set_position(PhysicalPosition::new(
            work.position.x + (i64::from(work.size.width) - width as i64) as i32 / 2,
            work.position.y
                + (i64::from(work.size.height) - height as i64 - (48.0 * scale) as i64).max(0)
                    as i32,
        ))?;
        window.set_size(tauri::PhysicalSize::new(width as u32, height as u32))?;
        window.set_zoom(zoom)?;
        let mut state = controller
            .state
            .lock()
            .map_err(|_| anyhow!("屏显状态异常"))?;
        if state.pending_sequence() != Some(sequence) {
            return Ok(());
        }
        window.show()?;
        let deadline = state
            .shown(sequence, controller.now())
            .ok_or_else(|| anyhow!("屏显已过期"))?;
        drop(state);
        let mut timer = controller
            .timer
            .lock()
            .map_err(|_| anyhow!("屏显计时状态异常"))?;
        if timer.is_none() {
            *timer = Some(Timer::new(app.clone())?);
        }
        timer.as_ref().unwrap().sender.send((sequence, deadline))?;
        emit(app);
        Ok(())
    })();
    if let Err(error) = result {
        report(app, sequence, error);
    }
}
fn hide(app: &AppHandle, sequence: u64) {
    let result = (|| -> Result<()> {
        let controller = app.state::<Controller>();
        let mut state = controller
            .state
            .lock()
            .map_err(|_| anyhow!("屏显状态异常"))?;
        if !state.expire(sequence, controller.now()) {
            return Ok(());
        }
        if let Some(window) = app.get_webview_window("osd") {
            window.hide()?;
        }
        drop(state);
        emit(app);
        Ok(())
    })();
    if let Err(error) = result {
        report(app, sequence, error);
    }
}
