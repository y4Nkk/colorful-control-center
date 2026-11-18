use super::{AppHandle, Manager, Record, Result, cancelled, keymap, mpsc, publish, store};
use crate::hotkeys::model::{NavigationKind, Status};
use anyhow::{Context, anyhow, bail};
use std::{
    path::Path,
    sync::{Arc, Mutex},
};
use tauri::Emitter;

fn show(app: &AppHandle) -> Result<()> {
    let window = app.get_webview_window("main").context("主窗口不可用")?;
    window.show().context("显示主窗口")?;
    window.unminimize().context("恢复主窗口")?;
    window.set_focus().context("聚焦主窗口")?;
    Ok(())
}
fn finish(app: &AppHandle, status: &Mutex<Status>, id: u64, result: Result<()>) {
    let Ok(mut state) = status.lock() else {
        log::error!("[hotkeys] 导航状态锁异常");
        return;
    };
    if !state.navigation_pending(id) {
        return;
    }
    match result {
        Ok(()) => {
            state.navigation_ready(id);
            state.error = None;
            state.last_result = Some("已请求定位设置区域".into());
        }
        Err(error) => {
            state.error = Some(format!("显示设置区域失败：{error:#}"));
            log::warn!("[hotkeys] 导航：{error:#}");
        }
    }
    if let Err(error) = app.emit_to("main", "hotkeys-state", state.clone()) {
        state.error = Some(format!("发送热键导航状态失败：{error}"));
        log::warn!("[hotkeys] 导航状态：{error}");
    }
}
pub(super) fn request(
    app: &AppHandle,
    path: &Path,
    approved: &Record,
    status: &Arc<Mutex<Status>>,
    stopping: &mpsc::Receiver<()>,
    kind: NavigationKind,
) -> Result<bool> {
    if cancelled(stopping) {
        return Ok(true);
    }
    if store::load(path)?.record != *approved {
        bail!("热键导航策略许可已撤销");
    }
    if app.state::<keymap::Controller>().recording_active()? {
        return Ok(false);
    }
    crate::hal::oem_owner::ensure_owner_absent()?;
    let id = status
        .lock()
        .map_err(|_| anyhow!("热键运行状态异常"))?
        .queue_navigation(kind)?;
    let handle = app.clone();
    let state = status.clone();
    if let Err(error) = app.run_on_main_thread(move || {
        let pending = state
            .lock()
            .map(|status| status.navigation_pending(id))
            .unwrap_or(false);
        if pending {
            let result = show(&handle);
            finish(&handle, &state, id, result);
        }
    }) {
        if let Ok(mut state) = status.lock() {
            state.error = Some(format!("请求显示设置区域失败：{error}"));
        }
        publish(app, status);
        return Err(error).context("请求显示设置区域");
    }
    Ok(false)
}

#[cfg(test)]
#[path = "navigation/tests.rs"]
mod tests;
