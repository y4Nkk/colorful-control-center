use super::{
    controller::Controller,
    runtime::Shared,
    worker::{ApplicationIo, Worker},
};
use crate::{
    hal::{
        Hal,
        power_events::{DisplayPolicy, Session},
    },
    powerctl::mode::profile::Grant,
    profiles::{
        self, ProfileFile,
        application::{self, Application, Origin, Permission},
    },
};
use anyhow::{Result, anyhow, bail};
use std::{sync::Arc, thread, time::Instant};
use tauri::Manager;

struct AppIo(tauri::AppHandle);
impl ApplicationIo for AppIo {
    fn begin(
        &self,
        file: &ProfileFile,
        mode: Option<&Grant>,
        origin: Origin,
        permission: Permission,
    ) -> Result<Application> {
        let app = self.0.clone();
        let original = permission.clone();
        let system = permission.clone();
        let permission = Permission::with_mode_driver(
            move || {
                original.check()?;
                if let Err(error) = app.state::<Hal>().ensure_automatic_profile_allowed() {
                    app.state::<Controller>().pause()?;
                    return Err(error);
                }
                Ok(())
            },
            move |facts| permission.bind_mode_driver(facts),
            move || system.require_system(),
        );
        let report = application::apply_automatic(&self.0, file, mode, origin, permission)?;
        application::log_application(&report);
        if report.pending.is_some() {
            crate::tray::show_main(&self.0);
        }
        Ok(report)
    }
    fn current(&self) -> Result<Option<Application>> {
        self.0
            .state::<application::Controller>()
            .read(&self.0.state::<crate::display::Controller>())
    }
    fn cancel(&self, id: u64) -> Result<Application> {
        application::cancel(&self.0, id)
    }
}

impl Controller {
    pub fn start(&self, app: &tauri::AppHandle) -> Result<()> {
        let mut worker = self
            .worker
            .lock()
            .map_err(|_| anyhow!("自动档案工作线程状态异常"))?;
        if worker.is_some() {
            bail!("自动档案工作线程已创建，未启动重复监听");
        }
        let app = app.clone();
        let shared = self.shared.clone();
        *worker = Some(
            thread::Builder::new()
                .name("power-profile-policy".into())
                .spawn(move || {
                    if let Err(error) = run(&shared, app) {
                        log::error!("[power-policy] {error:#}");
                        shared.failed(format!("{error:#}"));
                    }
                })?,
        );
        Ok(())
    }
}

fn run(shared: &Arc<Shared>, app: tauri::AppHandle) -> Result<()> {
    let policy =
        crate::platform::standby_display_policy(&app.state::<Hal>()).unwrap_or_else(|error| {
            log::warn!("[power-policy] Deep S5条件：{error:#}");
            DisplayPolicy::Unknown
        });
    let observed = shared.clone();
    let failed = shared.clone();
    let started = Instant::now();
    let mut listener = Session::start(
        policy,
        move |event| observed.observe(event, millis(started)),
        move |error| failed.failed(error),
    )?;
    *shared
        .stopper
        .lock()
        .map_err(|_| anyhow!("电源通知停止状态异常"))? = Some(listener.stopper());
    shared.ready(listener.status()?)?;
    let dir = profiles::profiles_dir(&app)?;
    let io = AppIo(app);
    let mut worker = Worker::default();
    let result = run_loop(shared, &dir, started, &listener, &io, &mut worker);
    let stopped = listener.stop();
    let finished = worker.finish(&io);
    *shared
        .stopper
        .lock()
        .map_err(|_| anyhow!("电源通知停止状态异常"))? = None;
    let mut issues = Vec::new();
    for (name, result) in [
        ("事件调度", result),
        ("通知退出", stopped),
        ("显示恢复", finished),
    ] {
        if let Err(error) = result {
            issues.push(format!("{name}：{error:#}"));
        }
    }
    if !issues.is_empty() {
        bail!("{}", issues.join("；"));
    }
    Ok(())
}
fn run_loop(
    shared: &Arc<Shared>,
    dir: &std::path::Path,
    started: Instant,
    listener: &Session,
    io: &AppIo,
    worker: &mut Worker,
) -> Result<()> {
    while shared.wait()? {
        if let Err(error) = listener.status() {
            log::error!("[power-policy] 电源通知：{error:#}");
            shared.failed(format!("{error:#}"));
            break;
        }
        if crate::startup::preparing(&io.0) {
            continue;
        }
        if let Err(error) = worker.tick(shared, dir, millis(started), io) {
            shared.issue(Some(format!("{error:#}")));
        }
    }
    Ok(())
}
fn millis(started: Instant) -> u64 {
    started.elapsed().as_millis().min(u64::MAX as u128) as u64
}
