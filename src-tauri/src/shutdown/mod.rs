pub(crate) mod deadline;
mod lifecycle;

use crate::{
    battery, display, fanctl, hal::Hal, hotkeys, osd, preferences, profiles, startup, tray,
};
use anyhow::{Result, anyhow, ensure};
use lifecycle::{Lifecycle, Operation};
use std::{process::Command, sync::OnceLock, thread, time::Duration};
use tauri::{AppHandle, Manager, RunEvent};

const BUDGET: Duration = Duration::from_secs(15);
pub(crate) const CLOSING: &str = "应用正在退出";
static LIFECYCLE: Lifecycle = Lifecycle::new();
static DEADLINE: OnceLock<deadline::Deadline> = OnceLock::new();

pub(crate) fn enter() -> Option<Operation<'static>> {
    LIFECYCLE.enter()
}

pub(crate) fn closing() -> bool {
    LIFECYCLE.closing()
}

pub(crate) fn request() {
    let _: Result<bool, std::convert::Infallible> = LIFECYCLE.request(|| {
        deadline::arm_or_terminate(DEADLINE.get(), BUDGET);
        Ok(())
    });
}

pub(crate) fn request_restart(commit: impl FnOnce() -> Result<()>) -> Result<()> {
    let requested = LIFECYCLE.request(|| {
        commit()?;
        deadline::arm_or_terminate(DEADLINE.get(), BUDGET);
        Ok::<_, anyhow::Error>(())
    })?;
    ensure!(requested, CLOSING);
    Ok(())
}

pub(crate) async fn blocking<F, T>(run: F) -> Result<T>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    tauri::async_runtime::spawn_blocking(move || {
        let _operation = enter().ok_or_else(|| anyhow!(CLOSING))?;
        Ok(run())
    })
    .await?
}

pub(crate) fn plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri::plugin::Builder::new("shutdown")
        .setup(|app, _| {
            start(app)?;
            if let Some(_operation) = enter() {
                tray::start(app)?;
            }
            Ok(())
        })
        .build()
}

fn start(app: &AppHandle) -> Result<()> {
    let mut command = Command::new(std::env::current_exe()?);
    command.arg(deadline::FLAG);
    DEADLINE
        .set(deadline::Deadline::launch(command)?)
        .map_err(|_| anyhow!("退出监护已经启动"))?;
    let app = app.clone();
    thread::Builder::new()
        .name("exit-cleanup".into())
        .spawn(move || {
            LIFECYCLE.wait_for_request();
            let code = clean_up(&app);
            LIFECYCLE.complete(code);
            app.exit(code);
        })?;
    Ok(())
}

pub(crate) fn on_event(_: &AppHandle, event: RunEvent) {
    match event {
        RunEvent::ExitRequested { api, .. } if LIFECYCLE.exit_code().is_none() => {
            api.prevent_exit();
            request();
        }
        RunEvent::Exit if LIFECYCLE.exit_code().is_none() => {
            // Windows Restart Manager 可直接结束事件循环。仍须等待同一收尾流程。
            request();
            let code = LIFECYCLE.wait_for_completion();
            if code != 0 {
                std::process::exit(code);
            }
        }
        _ => (),
    }
}

fn clean_up(app: &AppHandle) -> i32 {
    let mut failed = false;
    let mut record = |domain, result: Result<()>| {
        if let Err(error) = result {
            log::error!("[exit] {domain}：{error:#}");
            failed = true;
        }
    };
    record("主窗口唤回", startup::stop_activation(app));
    #[cfg(not(test))]
    let supply = app.try_state::<crate::powerctl::mode::supply::Controller>();
    #[cfg(not(test))]
    let refresh = app.try_state::<display::automatic::Controller>();
    let policy = app.try_state::<profiles::policy::Controller>();

    #[cfg(not(test))]
    if let Some(worker) = &supply {
        record("停止供电模式调度", worker.request_stop());
    }
    #[cfg(not(test))]
    if let Some(worker) = &refresh {
        record("停止自动刷新调度", worker.request_stop());
    }
    if let Some(worker) = &policy {
        record("停止自动档案调度", worker.request_stop());
    }
    if let Some(hal) = app.try_state::<Hal>() {
        record("停止软件风扇接管", hal.close_software_fan_session());
    }
    LIFECYCLE.drain();
    preferences::save_window_placement(app);

    #[cfg(not(test))]
    if let Some(worker) = supply {
        record("供电模式", worker.join_worker());
    }
    #[cfg(not(test))]
    if let Some(worker) = refresh {
        record("自动刷新率", worker.join_worker());
    }
    if let Some(worker) = policy {
        record("自动档案", worker.join_worker());
    }
    if let Some(controller) = app.try_state::<hotkeys::Controller>() {
        record("热键", controller.stop(app).map(|_| ()));
    }
    if let Some(controller) = app.try_state::<crate::keymap::Controller>() {
        record("停止宏录制", controller.stop_recording().map(|_| ()));
        record("停止宏监听", controller.release_programs_on_exit());
    }
    if let Some(controller) = app.try_state::<crate::lighting::perkey::Controller>() {
        record("停止逐键按键响应", controller.release_on_exit());
    }
    if let Some(controller) = app.try_state::<osd::Controller>() {
        controller.stop();
    }
    if let Some(hal) = app.try_state::<Hal>() {
        record("软件风扇", fanctl::release_on_exit(&hal));
        if let Some(controller) = app.try_state::<display::Controller>() {
            record("显示试用", controller.release_on_exit(&hal));
        }
        if let Some(controller) = app.try_state::<battery::Controller>() {
            record("电池校准", controller.release_calibration_on_exit(&hal));
        }
        while hal.fan_recovery_pending() {
            thread::sleep(Duration::from_millis(50));
        }
    }
    record("托盘", tray::stop(app));
    i32::from(failed)
}
