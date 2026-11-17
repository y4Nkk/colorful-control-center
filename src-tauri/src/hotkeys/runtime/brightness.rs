use super::{AppHandle, Manager, Record, Result, actions, cancelled, mpsc, store};
use anyhow::bail;
use std::time::Duration;
pub(super) fn execute(
    app: &AppHandle,
    path: &std::path::Path,
    approved: &Record,
    stopping: &mpsc::Receiver<()>,
    identity: &str,
    event: u8,
) -> (Result<actions::Completion>, bool) {
    let stopped = std::cell::Cell::new(false);
    let permit = || {
        if cancelled(stopping) {
            stopped.set(true);
            return Ok(false);
        }
        if store::load(path)?.record != *approved {
            bail!("屏幕亮度策略许可已撤销");
        }
        crate::hal::oem_owner::ensure_owner_absent()?;
        Ok(true)
    };
    let wait = || match stopping.recv_timeout(Duration::from_secs(1)) {
        Err(mpsc::RecvTimeoutError::Timeout) => Ok(true),
        _ => {
            stopped.set(true);
            Ok(false)
        }
    };
    let result = app
        .state::<crate::hotkeys::Controller>()
        .screen_brightness
        .run(
            &app.state::<crate::hal::Hal>(),
            identity,
            event,
            permit,
            wait,
        )
        .map(|message| actions::Completion {
            message,
            show_osd: true,
        });
    if cancelled(stopping) {
        stopped.set(true);
    }
    (result, stopped.get())
}
