use super::{Controller, control::source};
use crate::{
    display, gpu_power,
    gpuctl::GpuController,
    hal::{
        Hal,
        power_events::{DisplayPolicy, Session},
    },
};
use anyhow::{Result, anyhow, bail};
use std::{
    thread,
    time::Duration,
};
use tauri::{Emitter, Manager};

impl Controller {
    pub fn start_worker(&self, app: &tauri::AppHandle) -> Result<()> {
        let mut worker = self
            .worker
            .lock()
            .map_err(|_| anyhow!("供电模式工作线程状态异常"))?;
        if worker.is_some() {
            bail!("供电模式监听已启动");
        }
        let app = app.clone();
        let shared = self.shared.clone();
        *worker = Some(
            thread::Builder::new()
                .name("oem-supply-mode".into())
                .spawn(move || {
                    if let Err(error) = run(&app) {
                        log::error!("[supply-mode] {error:#}");
                        shared.failed(format!("{error:#}"));
                    }
                })?,
        );
        Ok(())
    }
    pub fn request_stop(&self) -> Result<()> {
        self.shared.lock()?.scheduler.stop()?;
        self.shared.wake.notify_all();
        if let Some(stopper) = self
            .shared
            .stopper
            .lock()
            .map_err(|_| anyhow!("供电模式监听停止状态异常"))?
            .as_ref()
        {
            stopper.stop();
        }
        Ok(())
    }

    pub fn join_worker(&self) -> Result<()> {
        let worker = self
            .worker
            .lock()
            .map_err(|_| anyhow!("供电模式工作线程状态异常"))?
            .take();
        if let Some(worker) = worker {
            worker
                .join()
                .map_err(|_| anyhow!("供电模式工作线程退出异常"))?;
        }
        Ok(())
    }
}
fn run(app: &tauri::AppHandle) -> Result<()> {
    let _restriction = crate::hal::authorization::prompt::Restriction::enter(false);
    let controller = app.state::<Controller>();
    let observed = controller.shared.clone();
    let failed = controller.shared.clone();
    let mut listener = Session::start(
        DisplayPolicy::Unknown,
        move |event| observed.observe(event, display::now_ms()),
        move |error| failed.failed(error),
    )?;
    *controller
        .shared
        .stopper
        .lock()
        .map_err(|_| anyhow!("供电模式监听停止状态异常"))? = Some(listener.stopper());
    let result = (|| {
        listener.status()?;
        controller.shared.lock()?.scheduler.ready()?;
        run_loop(app, &listener)
    })();
    let stopped = listener.stop();
    *controller
        .shared
        .stopper
        .lock()
        .map_err(|_| anyhow!("供电模式监听停止状态异常"))? = None;
    match (result, stopped) {
        (Err(error), Err(stopped)) => bail!("{error:#}；供电通知退出失败：{stopped:#}"),
        (Err(error), _) | (_, Err(error)) => Err(error),
        _ => Ok(()),
    }
}
fn run_loop(app: &tauri::AppHandle, listener: &Session) -> Result<()> {
    let controller = app.state::<Controller>();
    let mut initialized = false;
    loop {
        let state = controller.shared.lock()?;
        if state.scheduler.stopping {
            return Ok(());
        }
        let (state, _) = controller
            .shared
            .wake
            .wait_timeout(state, Duration::from_millis(500))
            .map_err(|_| anyhow!("供电模式事件等待异常"))?;
        if state.scheduler.stopping {
            return Ok(());
        }
        drop(state);
        if crate::shutdown::closing() {
            return Ok(());
        }
        listener.status()?;
        if crate::startup::profile_in_progress(app)? {
            continue;
        }
        if !initialized {
            initialized = true;
            if let Err(error) = controller.resume_saved(
                &app.state::<Hal>(),
                &app.state::<GpuController>(),
                &app.state::<gpu_power::Controller>(),
                &app.state::<display::Controller>(),
            ) {
                controller.shared.pause(Some(format!("{error:#}")))?;
            }
        }
        if let Err(error) = tick(app) {
            controller.shared.pause(Some(format!("{error:#}")))?;
            log::warn!("[supply-mode] {error:#}");
        }
    }
}
fn tick(app: &tauri::AppHandle) -> Result<()> {
    let controller = app.state::<Controller>();
    {
        let state = controller.shared.lock()?;
        if !state.scheduler.enabled || !state.scheduler.online || state.scheduler.sleeping {
            return Ok(());
        }
    }
    let hal = app.state::<Hal>();
    let clocks = app.state::<GpuController>();
    let power = app.state::<gpu_power::Controller>();
    let display = app.state::<display::Controller>();
    let work = {
        let _operation = controller.lock()?;
        controller.next_work(
            source(crate::battery::system_power()?.ac_online),
            display::now_ms(),
            display.trial_pending()?,
        )?
    };
    if let Some(work) = work {
        let result = controller.execute_work(
            &hal,
            &clocks,
            &power,
            &display,
            &app.state::<display::automatic::Controller>(),
            work,
        );
        if let Err(error) = app.emit("hardware-controls-changed", ()) {
            log::warn!("[supply-mode] 控制状态通知：{error}");
        }
        result?;
    }
    Ok(())
}
