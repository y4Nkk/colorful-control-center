use super::{actions, model::*, store};
use crate::{hal::event_log, keymap, osd};
use anyhow::{Context, Result, anyhow, bail};
use ccc_core::oem_events::{self, Action, Gate};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex, mpsc},
    thread,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, Manager};
#[path = "runtime/airplane.rs"]
mod airplane;
#[path = "runtime/battery_events.rs"]
mod battery_events;
#[path = "runtime/battery_thresholds.rs"]
mod battery_thresholds;
#[path = "runtime/brightness.rs"]
mod brightness;
#[path = "runtime/low_action.rs"]
mod low_action;
#[path = "runtime/low_battery.rs"]
mod low_battery;
#[path = "runtime/mode.rs"]
mod mode;
#[path = "runtime/navigation.rs"]
mod navigation;
#[path = "runtime/panel_event.rs"]
mod panel_event;
#[path = "runtime/pd_event.rs"]
mod pd_event;
#[path = "runtime/recovery.rs"]
mod recovery;
#[path = "runtime/report.rs"]
mod report;
#[path = "runtime/system_action.rs"]
mod system_action;
use report::{publish, report};

#[path = "runtime/session.rs"]
mod session;
pub(super) use session::Session;
#[path = "runtime/battery_alert.rs"]
mod battery_alert;

pub(super) fn packet(record: &event_log::Record) -> Result<Option<Action>> {
    if record.source != "PowerBIOSServer_Out" {
        return Ok(None);
    }
    if record.strings.len() != 1 {
        bail!("原厂热键事件插入字符串数量无效");
    }
    oem_events::decode(&record.strings[0])
        .map(|p| p.and_then(|p| p.action()))
        .map_err(anyhow::Error::msg)
}
fn consume(
    app: &AppHandle,
    path: &std::path::Path,
    approved: &mut Record,
    status: &Arc<Mutex<Status>>,
    stopping: &mpsc::Receiver<()>,
    log: &mut event_log::Log,
    battery_events: &mut battery_events::Source,
) -> Result<()> {
    let clock = Instant::now();
    panel_event::startup(app, path, approved, stopping, status)?;
    let mut gate = Gate::default();
    loop {
        match stopping.recv_timeout(Duration::from_millis(80)) {
            Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(()),
            Err(mpsc::RecvTimeoutError::Timeout) => (),
        }
        if battery_alert::tick(app, path, approved, stopping)? {
            return Ok(());
        }
        if tick_panel(app, path, approved, status, stopping)? {
            return Ok(());
        }
        if battery_events::tick(app, path, approved, status, stopping, battery_events)? {
            return Ok(());
        }
        for record in log.poll()? {
            if consume_packet(
                app,
                path,
                approved,
                status,
                stopping,
                &record,
                &mut gate,
                clock.elapsed(),
            )? {
                return Ok(());
            }
        }
    }
}
fn consume_packet(
    app: &AppHandle,
    path: &std::path::Path,
    approved: &mut Record,
    status: &Arc<Mutex<Status>>,
    stopping: &mpsc::Receiver<()>,
    record: &event_log::Record,
    gate: &mut Gate,
    elapsed: Duration,
) -> Result<bool> {
    if cancelled(stopping) {
        return Ok(true);
    }
    let Some(action) = packet(record)? else {
        return Ok(false);
    };
    if !approved
        .settings
        .actions
        .contains(&Scope::for_action(action))
        || !gate.accept(action, elapsed)
    {
        return Ok(false);
    }
    if app.state::<keymap::Controller>().recording_active()? {
        return Ok(false);
    }
    if store::load(path)?.record != *approved {
        bail!("热键策略文件已变化，监听已停止，请重新确认");
    }
    let scope = Scope::for_action(action);
    let facts = actions::facts(app, &[scope]);
    approved.verify_action(&facts, scope)?;
    if cancelled(stopping) {
        return Ok(true);
    }
    let identity = facts
        .capabilities
        .iter()
        .find(|c| c.scope == Scope::for_action(action))
        .and_then(|c| c.identity.as_deref())
        .context("热键能力不可用")?;
    if action == Action::BatteryColdWarning {
        return battery_events::handle(
            app,
            path,
            approved,
            status,
            stopping,
            identity,
            super::battery_cold::Trigger::Event,
        );
    }
    if let Some(kind) = match action {
        Action::OpenKeyboard => Some(NavigationKind::Keyboard),
        Action::DisplayRotation => Some(NavigationKind::DisplayRotation),
        _ => None,
    } {
        return navigation::request(app, path, approved, status, stopping, kind);
    }
    let (result, stopped) = execute(app, path, approved, stopping, action, identity);
    if stopped {
        return result.map(|_| true);
    }
    let Some((message, show_osd)) = completed_message(result, action) else {
        return Ok(false);
    };
    if action.is_status_feedback() && report::feedback_cancelled(path, approved, stopping)? {
        return Ok(true);
    }
    report(
        app,
        status,
        action,
        approved.settings.osd && show_osd,
        message,
    );
    Ok(false)
}
fn execute(
    app: &AppHandle,
    path: &std::path::Path,
    approved: &mut Record,
    stopping: &mpsc::Receiver<()>,
    action: Action,
    identity: &str,
) -> (Result<actions::Completion>, bool) {
    if action == Action::ToggleAirplane {
        return airplane::execute(app, path, approved, stopping, identity);
    }
    if action == Action::PanelEvent {
        return panel_event::execute(app, path, approved, stopping, identity, false);
    }
    if let Action::SystemShutdown | Action::LidAction = action {
        return system_action::execute(app, path, approved, stopping, identity, action);
    }
    if action == Action::OpenRecovery {
        return recovery::execute(app, path, approved, stopping, identity);
    }
    if action == Action::CycleMode {
        return mode::execute(app, path, approved, stopping, identity);
    }
    if action == Action::PanelHotkey {
        return execute_panel(app, path, approved, stopping, identity);
    }
    if let Action::GpuTurbo(event) = action {
        return execute_gpu(app, path, approved, stopping, identity, event);
    }
    if let Action::BatteryThreshold(event) = action {
        return battery_thresholds::execute(app, path, approved, stopping, identity, event);
    }
    if let Action::BatteryAlert { active, brightness } = action {
        return battery_alert::execute(app, path, approved, stopping, identity, active, brightness);
    }
    if let Action::BatteryLowBrightness(event) = action {
        return low_battery::execute(app, path, approved, stopping, identity, event);
    }
    if let Action::BatteryLowAction(event) = action {
        return low_action::execute(app, path, approved, stopping, identity, event);
    }
    if action == Action::PowerDeliveryEvent {
        return pd_event::execute(app, path, approved, stopping, identity);
    }
    let Action::DisplayBrightness(event) = action else {
        return (actions::execute(app, action, identity), false);
    };
    brightness::execute(app, path, approved, stopping, identity, event)
}
fn execute_gpu(
    app: &AppHandle,
    path: &std::path::Path,
    approved: &Record,
    stopping: &mpsc::Receiver<()>,
    identity: &str,
    event: u8,
) -> (Result<actions::Completion>, bool) {
    let stopped = std::cell::Cell::new(false);
    let permit = || -> Result<()> {
        if cancelled(stopping) {
            stopped.set(true);
            bail!("GPU Turbo 监听已停止");
        }
        if store::load(path)?.record != *approved {
            bail!("GPU Turbo 策略许可已撤销");
        }
        if app.state::<keymap::Controller>().recording_active()? {
            bail!("按键录制中，GPU Turbo 执行已取消");
        }
        crate::hal::oem_owner::ensure_owner_absent()
    };
    let result = app
        .state::<crate::gpuctl::GpuController>()
        .run_turbo(&app.state::<crate::hal::Hal>(), identity, event, permit)
        .map(|detail| actions::Completion {
            message: Some(osd::Message::result("原厂 GPU Turbo", detail)),
            show_osd: true,
        });
    if cancelled(stopping) {
        stopped.set(true);
    }
    (result, stopped.get())
}
fn execute_panel(
    app: &AppHandle,
    path: &std::path::Path,
    approved: &Record,
    stopping: &mpsc::Receiver<()>,
    identity: &str,
) -> (Result<actions::Completion>, bool) {
    let stopped = std::cell::Cell::new(false);
    let permit = || panel_permit(app, path, approved, stopping, &stopped);
    let wait = || match stopping.recv_timeout(Duration::from_millis(100)) {
        Err(mpsc::RecvTimeoutError::Timeout) => Ok(true),
        _ => {
            stopped.set(true);
            Ok(false)
        }
    };
    let result = app
        .state::<super::Controller>()
        .panel_hotkey
        .run(&app.state::<crate::hal::Hal>(), identity, permit, wait)
        .map(|message| actions::Completion {
            message: message.map(|detail| osd::Message::result("原厂屏幕按键", detail)),
            show_osd: false,
        });
    if cancelled(stopping) {
        stopped.set(true);
    }
    (result, stopped.get())
}
fn panel_permit(
    app: &AppHandle,
    path: &std::path::Path,
    approved: &Record,
    stopping: &mpsc::Receiver<()>,
    stopped: &std::cell::Cell<bool>,
) -> Result<bool> {
    if cancelled(stopping) {
        stopped.set(true);
        return Ok(false);
    }
    if store::load(path)?.record != *approved {
        bail!("屏幕按键策略许可已撤销");
    }
    if app.state::<keymap::Controller>().recording_active()? {
        return Ok(false);
    }
    approved.verify_action(
        &actions::facts(app, &[Scope::PanelHotkey]),
        Scope::PanelHotkey,
    )?;
    Ok(true)
}
fn tick_panel(
    app: &AppHandle,
    path: &std::path::Path,
    approved: &Record,
    status: &Mutex<Status>,
    stopping: &mpsc::Receiver<()>,
) -> Result<bool> {
    let controller = app.state::<super::Controller>();
    if !controller.panel_hotkey.active()? {
        return Ok(false);
    }
    if store::load(path)?.record != *approved {
        bail!("屏幕按键策略文件已变化，正在结束本软件会话");
    }
    crate::hal::oem_owner::ensure_owner_absent()?;
    let stopped = std::cell::Cell::new(false);
    let result = controller
        .panel_hotkey
        .tick(&app.state::<crate::hal::Hal>(), || {
            panel_permit(app, path, approved, stopping, &stopped)
        })
        .map(|message| actions::Completion {
            message: message.map(|detail| osd::Message::result("原厂屏幕按键", detail)),
            show_osd: false,
        });
    if let Some((message, _)) = completed_message(result, Action::PanelHotkey) {
        report(app, status, Action::PanelHotkey, false, message);
    }
    Ok(stopped.get())
}
fn completed_message(
    result: Result<actions::Completion>,
    action: Action,
) -> Option<(osd::Message, bool)> {
    match result {
        Ok(completion) => completion
            .message
            .map(|message| (message, completion.show_osd)),
        Err(error) => {
            let title = if action.is_status_feedback() {
                "状态提示未显示"
            } else {
                "操作未完成"
            };
            let mut message = osd::Message::result(title, format!("{error:#}"));
            message.error = true;
            Some((message, action != Action::PanelHotkey))
        }
    }
}
fn cancelled(stopping: &mpsc::Receiver<()>) -> bool {
    matches!(
        stopping.try_recv(),
        Ok(()) | Err(mpsc::TryRecvError::Disconnected)
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn panel_failure_updates_status_without_overlaying_the_blank_saver() {
        let panel = completed_message(Err(anyhow!("测试失败")), Action::PanelHotkey).unwrap();
        let ordinary = completed_message(Err(anyhow!("测试失败")), Action::CycleMode).unwrap();
        assert!(panel.0.error && !panel.1);
        assert!(ordinary.0.error && ordinary.1);
    }
}
