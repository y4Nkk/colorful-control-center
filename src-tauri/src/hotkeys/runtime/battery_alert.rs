use super::*;
use std::{cell::Cell, path::Path};

pub(super) fn execute(
    app: &AppHandle,
    path: &Path,
    approved: &Record,
    stopping: &mpsc::Receiver<()>,
    identity: &str,
    active: bool,
    brightness: Option<u8>,
) -> (Result<actions::Completion>, bool) {
    let stopped = Cell::new(false);
    let controller = app.state::<crate::hotkeys::Controller>();
    let result = controller
        .battery_alert
        .event(
            &app.state::<crate::hal::Hal>(),
            &controller.screen_brightness,
            identity,
            active,
            brightness,
            || low_battery::permit(app, path, approved, stopping, &stopped),
        )
        .map(|_| actions::Completion {
            message: Some(osd::Message::result(
                "原厂电池音频事件",
                if active {
                    "已处理开始事件，初始亮度仍可单独恢复"
                } else {
                    "已停止静音循环；原厂已排队通知仍按条件处理"
                },
            )),
            show_osd: false,
        });
    let stop_session = stopped.get() || result.is_err();
    (result, stop_session)
}
pub(super) fn tick(
    app: &AppHandle,
    path: &Path,
    approved: &Record,
    stopping: &mpsc::Receiver<()>,
) -> Result<bool> {
    if !approved.settings.actions.contains(&Scope::BatteryAlert) {
        return Ok(false);
    }
    let stopped = Cell::new(false);
    let controller = app.state::<crate::hotkeys::Controller>();
    let result = controller.battery_alert.tick(
        &app.state::<crate::hal::Hal>(),
        &controller.screen_brightness,
        || low_battery::permit(app, path, approved, stopping, &stopped),
    );
    if stopped.get() {
        return Ok(true);
    }
    result.map(|_| false)
}
