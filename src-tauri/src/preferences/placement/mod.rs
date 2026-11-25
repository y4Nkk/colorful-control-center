mod geometry;
mod native;
#[cfg(test)]
mod tests;

use anyhow::{Context, Result, ensure};
use geometry::SavedPlacement;
use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use tauri::{AppHandle, Manager, WebviewWindow, utils::config::WindowConfig};

const FILE_NAME: &str = "main-window-placement.json";
type LatestPlacement = Mutex<Option<SavedPlacement>>;

struct Store {
    path: PathBuf,
    latest: Arc<LatestPlacement>,
}

pub(super) fn bind(window: &WebviewWindow, config: &WindowConfig) -> Result<()> {
    let path = match window.app_handle().path().app_local_data_dir() {
        Ok(dir) => Some(dir.join(FILE_NAME)),
        Err(error) => {
            log::warn!("[window] 获取窗口布局目录失败：{error}");
            None
        }
    };
    bind_at(window, config, path)
}

fn bind_at(window: &WebviewWindow, config: &WindowConfig, path: Option<PathBuf>) -> Result<()> {
    let hwnd = window.hwnd()?.0;
    let latest = Arc::new(Mutex::new(None));
    if let Err(error) = native::track(hwnd, &latest) {
        log::warn!("[window] {error:#}");
    }
    let saved = path.as_deref().and_then(|path| match read(path) {
        Ok(saved) => saved,
        Err(error) => {
            log::warn!("[window] 读取窗口布局失败：{error:#}");
            None
        }
    });
    if let Err(error) = native::restore(window, hwnd, config, saved.as_ref()) {
        log::warn!("[window] 恢复主窗口布局失败：{error:#}");
    }
    if let Some(path) = path {
        let initial = native::capture(hwnd, false);
        if let Ok(mut snapshot) = latest.lock() {
            *snapshot = initial;
        }
        window.app_handle().manage(Store { path, latest });
    }
    Ok(())
}

pub(crate) fn save(app: &AppHandle) {
    if let Some(store) = app.try_state::<Store>()
        && let Err(error) = store.save()
    {
        log::warn!("[window] 保存窗口布局失败：{error:#}");
    }
}

impl Store {
    fn save(&self) -> Result<()> {
        let snapshot = self
            .latest
            .lock()
            .map_err(|_| anyhow::anyhow!("窗口布局状态不可用"))?
            .clone();
        if let Some(saved) = snapshot {
            let dir = self.path.parent().context("窗口布局目录无效")?;
            fs::create_dir_all(dir)?;
            crate::storage::write(&self.path, &serde_json::to_vec(&saved)?)?;
        }
        Ok(())
    }
}

fn read(path: &Path) -> Result<Option<SavedPlacement>> {
    let data = match fs::read(path) {
        Ok(data) => data,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let saved: SavedPlacement = serde_json::from_slice(&data)?;
    ensure!(saved.valid(), "窗口布局包含无效的尺寸或坐标");
    Ok(Some(saved))
}
