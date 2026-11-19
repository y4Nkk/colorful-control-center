use super::{
    controller::{Controller, Shared, permit_recovery},
    fields::FileStore,
    native::Native,
    state,
};
use crate::{
    display,
    hal::{
        Hal,
        power_events::{DisplayPolicy, Session},
    },
};
use anyhow::{Result, anyhow, bail};
use ccc_core::oem_refresh::Trigger;
use std::{
    sync::Arc,
    thread,
    time::Duration,
};
use tauri::Manager;

mod worker;

impl Controller {
    pub fn start_worker(&self, app: &tauri::AppHandle) -> Result<()> {
        let mut worker = self
            .worker
            .lock()
            .map_err(|_| anyhow!("自动刷新率工作线程状态异常"))?;
        if worker.is_some() {
            bail!("自动刷新率监听已启动");
        }
        let shared = self.shared.clone();
        let store = self.store.clone();
        let app = app.clone();
        *worker = Some(
            thread::Builder::new()
                .name("oem-auto-refresh".into())
                .spawn(move || {
                    if let Err(error) = run(&shared, &store, &app) {
                        log::error!("[auto-refresh] {error:#}");
                        shared.fail(format!("{error:#}"));
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
            .map_err(|_| anyhow!("电源监听停止状态异常"))?
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
            .map_err(|_| anyhow!("自动刷新率工作线程状态异常"))?
            .take();
        if let Some(worker) = worker {
            worker
                .join()
                .map_err(|_| anyhow!("自动刷新率工作线程退出异常"))?;
        }
        Ok(())
    }
}
fn run(shared: &Arc<Shared>, store: &FileStore, app: &tauri::AppHandle) -> Result<()> {
    let observed = shared.clone();
    let failed = shared.clone();
    let mut listener = Session::start(
        DisplayPolicy::Unknown,
        move |event| observed.observe(event, display::now_ms()),
        move |error| failed.fail(error),
    )?;
    *shared
        .stopper
        .lock()
        .map_err(|_| anyhow!("电源监听停止状态异常"))? = Some(listener.stopper());
    let result = (|| {
        listener.status()?;
        shared.lock()?.scheduler.ready()?;
        run_loop(shared, store, app, &listener)
    })();
    let stopped = listener.stop();
    let recovered = worker::cancel_on_exit(shared, store, app);
    *shared
        .stopper
        .lock()
        .map_err(|_| anyhow!("电源监听停止状态异常"))? = None;
    let mut issues = Vec::new();
    for (context, result) in [
        ("自动刷新率监听", result),
        ("通知退出", stopped),
        ("显示恢复", recovered),
    ] {
        if let Err(error) = result {
            issues.push(format!("{context}：{error:#}"));
        }
    }
    if !issues.is_empty() {
        bail!("{}", issues.join("；"));
    }
    Ok(())
}
fn resume(shared: &Shared, store: &FileStore, app: &tauri::AppHandle) {
    let result = (|| {
        if store.load()?.is_none_or(|record| !record.active()) {
            return Ok(());
        }
        let hal = app.state::<Hal>();
        let display = app.state::<display::Controller>();
        let _gate = hal.begin_write()?;
        let view = state::read(
            &Native {
                hal: &hal,
                display: &display,
            },
            store,
        )?;
        if let Some(error) = view.error {
            bail!("{error}");
        }
        if view.active {
            let facts = view
                .current
                .ok_or_else(|| anyhow!("自动刷新率事实不可用"))?;
            let supply = facts.on_ac.ok_or_else(|| anyhow!("供电状态未知"))?;
            permit_recovery(&hal)?;
            shared.lock()?.scheduler.configure(
                facts.mode,
                supply,
                Trigger::Startup,
                display::now_ms(),
            )?;
        }
        Ok(())
    })();
    if let Err(error) = result {
        shared.issue(Some(format!("{error:#}")));
    }
}
fn run_loop(
    shared: &Shared,
    store: &FileStore,
    app: &tauri::AppHandle,
    listener: &Session,
) -> Result<()> {
    let mut initialized = false;
    loop {
        let state = shared.lock()?;
        if state.scheduler.stopping {
            break;
        }
        let (state, _) = shared
            .wake
            .wait_timeout(state, Duration::from_millis(500))
            .map_err(|_| anyhow!("自动刷新率等待状态异常"))?;
        if state.scheduler.stopping {
            break;
        }
        drop(state);
        if crate::shutdown::closing() {
            return Ok(());
        }
        if let Err(error) = listener.status() {
            shared.fail(format!("{error:#}"));
            return Err(error);
        }
        if !initialized {
            if crate::startup::profile_in_progress(app)? {
                continue;
            }
            initialized = true;
            resume(shared, store, app);
        }
        if let Err(error) = worker::tick(shared, store, app) {
            let message = format!("{error:#}");
            if worker::pause(shared, message.clone())? {
                log::warn!("[auto-refresh] {message}");
            }
        }
    }
    Ok(())
}
