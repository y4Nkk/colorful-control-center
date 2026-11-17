use super::{
    AppHandle, Manager, Record, Result, actions, cancelled, keymap, mpsc, osd, report, store,
};
use crate::hotkeys::model::Status;
use anyhow::bail;
use ccc_core::oem_events::Action;
use std::{cell::Cell, path::Path, sync::Mutex};

pub(super) fn permit(
    app: &AppHandle,
    path: &Path,
    approved: &Record,
    stopping: &mpsc::Receiver<()>,
    stopped: &Cell<bool>,
) -> Result<()> {
    if cancelled(stopping) {
        stopped.set(true);
        bail!("电池事件监听已停止");
    }
    if store::load(path)?.record != *approved {
        bail!("电池事件策略许可已撤销");
    }
    if app.state::<keymap::Controller>().recording_active()? {
        bail!("按键录制中，电池事件执行已取消");
    }
    crate::hal::oem_owner::ensure_owner_absent()
}
pub(super) fn execute(
    app: &AppHandle,
    path: &Path,
    approved: &Record,
    stopping: &mpsc::Receiver<()>,
    identity: &str,
    event: u8,
) -> (Result<actions::Completion>, bool) {
    let stopped = Cell::new(false);
    let result = app
        .state::<crate::hotkeys::Controller>()
        .screen_brightness
        .run_low(&app.state::<crate::hal::Hal>(), identity, event, || {
            permit(app, path, approved, stopping, &stopped)
        })
        .map(|detail| actions::Completion {
            message: Some(osd::Message::result("原厂低电量亮度", detail)),
            show_osd: false,
        });
    if cancelled(stopping) {
        stopped.set(true);
    }
    (result, stopped.get())
}
pub(super) fn check(
    app: &AppHandle,
    path: &Path,
    approved: &Record,
    status: &Mutex<Status>,
    stopping: &mpsc::Receiver<()>,
    identity: &str,
) -> Result<bool> {
    let stopped = Cell::new(false);
    let result = app
        .state::<crate::hotkeys::Controller>()
        .screen_brightness
        .check_low(&app.state::<crate::hal::Hal>(), identity, || {
            permit(app, path, approved, stopping, &stopped)
        });
    if stopped.get() || cancelled(stopping) {
        return Ok(true);
    }
    match result {
        Ok(Some(detail)) => report(
            app,
            status,
            Action::BatteryLowBrightness(60),
            false,
            osd::Message::result("原厂低电量亮度", detail),
        ),
        Ok(None) => (),
        Err(error) => {
            let mut message = osd::Message::result("低电量亮度恢复不可用", format!("{error:#}"));
            message.error = true;
            report(
                app,
                status,
                Action::BatteryLowBrightness(60),
                false,
                message,
            );
        }
    }
    Ok(false)
}
