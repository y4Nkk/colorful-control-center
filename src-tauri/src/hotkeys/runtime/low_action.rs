use super::{AppHandle, Manager, Record, Result, actions, cancelled, low_battery, mpsc, osd};
use std::{cell::Cell, path::Path, time::Duration};

pub(super) fn execute(
    app: &AppHandle,
    path: &Path,
    approved: &Record,
    stopping: &mpsc::Receiver<()>,
    identity: &str,
    event: u8,
) -> (Result<actions::Completion>, bool) {
    let stopped = Cell::new(false);
    let permit = || low_battery::permit(app, path, approved, stopping, &stopped);
    let wait = || match stopping.recv_timeout(Duration::from_secs(1)) {
        Err(mpsc::RecvTimeoutError::Timeout) => Ok(true),
        _ => {
            stopped.set(true);
            Ok(false)
        }
    };
    let result = crate::hotkeys::low_battery_action::execute(
        &app.state::<crate::hal::Hal>(),
        identity,
        event,
        permit,
        wait,
    )
    .map(|message| actions::Completion {
        message: message.map(|detail| osd::Message::result("原厂低电量系统动作", detail)),
        show_osd: false,
    });
    if cancelled(stopping) {
        stopped.set(true);
    }
    (result, stopped.get())
}
