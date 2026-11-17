use super::{AppHandle, Manager, Record, Result, actions, cancelled, keymap, mpsc, osd, store};
use anyhow::bail;
use std::{cell::Cell, path::Path};

pub(super) fn execute(
    app: &AppHandle,
    path: &Path,
    approved: &Record,
    stopping: &mpsc::Receiver<()>,
    identity: &str,
) -> (Result<actions::Completion>, bool) {
    let stopped = Cell::new(false);
    let result = crate::hotkeys::system_recovery::execute(identity, || {
        if cancelled(stopping) {
            stopped.set(true);
            bail!("系统恢复入口请求已取消");
        }
        if store::load(path)?.record != *approved {
            bail!("系统恢复入口许可已撤销");
        }
        if app.state::<keymap::Controller>().recording_active()? {
            bail!("按键录制中，未打开Windows恢复");
        }
        crate::hal::oem_owner::ensure_owner_absent()
    })
    .map(|_| actions::Completion {
        message: Some(osd::Message::result("Windows恢复", "已请求打开恢复选项")),
        show_osd: false,
    });
    (result, stopped.get())
}
