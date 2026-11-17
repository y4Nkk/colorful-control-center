use super::{AppHandle, Emitter, Manager, Record, Result, Status, cancelled, mpsc, osd, store};
use anyhow::bail;
use ccc_core::oem_events::Action;
use std::{path::Path, sync::Mutex};

pub(super) fn feedback_cancelled(
    path: &Path,
    approved: &Record,
    stopping: &mpsc::Receiver<()>,
) -> Result<bool> {
    if cancelled(stopping) {
        return Ok(true);
    }
    if store::load(path)?.record != *approved {
        bail!("热键策略文件已变化，状态提示已取消，请重新确认");
    }
    crate::hal::oem_owner::ensure_owner_absent()?;
    Ok(false)
}

pub(super) fn publish(app: &AppHandle, status: &Mutex<Status>) {
    if let Ok(status) = status.lock() {
        if let Err(error) = app.emit_to("main", "hotkeys-state", status.clone()) {
            log::warn!("[hotkeys] 状态通知：{error}");
        }
    }
}
pub(super) fn report(
    app: &AppHandle,
    status: &Mutex<Status>,
    action: Action,
    show_osd: bool,
    message: osd::Message,
) {
    if let Ok(mut state) = status.lock() {
        state.last_result = Some(format!("{}：{}", message.title, message.detail));
        state.error = message.error.then(|| message.detail.clone());
    }
    publish(app, status);
    if !action.is_status_feedback() {
        if let Err(error) = app.emit_to("main", "hardware-controls-changed", ()) {
            log::warn!("[hotkeys] 刷新通知：{error}");
        }
    }
    if show_osd {
        if let Err(error) = app.state::<osd::Controller>().show(app, message) {
            log::error!("[osd] {error:#}");
        }
    }
}
