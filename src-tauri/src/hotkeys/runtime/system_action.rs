use super::{AppHandle, Manager, Record, Result, actions, cancelled, keymap, mpsc, osd, store};
use anyhow::bail;
use ccc_core::oem_events::Action;
use std::{cell::Cell, path::Path};

pub(super) fn execute(
    app: &AppHandle,
    path: &Path,
    approved: &Record,
    stopping: &mpsc::Receiver<()>,
    identity: &str,
    action: Action,
) -> (Result<actions::Completion>, bool) {
    let stopped = Cell::new(false);
    let event = if action == Action::SystemShutdown {
        72
    } else {
        192
    };
    let permit = || {
        if cancelled(stopping) {
            stopped.set(true);
            bail!("系统电源事件接管已停止");
        }
        if store::load(path)?.record != *approved {
            bail!("系统电源事件许可已撤销");
        }
        if app.state::<keymap::Controller>().recording_active()? {
            bail!("按键录制中，系统电源事件已取消");
        }
        crate::hal::oem_owner::ensure_owner_absent()
    };
    let wait = |delay| match stopping.recv_timeout(delay) {
        Err(mpsc::RecvTimeoutError::Timeout) => Ok(true),
        _ => {
            stopped.set(true);
            Ok(false)
        }
    };
    let result = crate::hotkeys::system_action::execute(
        &app.state::<crate::hal::Hal>(),
        event,
        identity,
        permit,
        wait,
    )
    .map(|message| actions::Completion {
        message: message.map(|text| osd::Message::result("系统电源", text)),
        show_osd: false,
    });
    (result, stopped.get())
}
