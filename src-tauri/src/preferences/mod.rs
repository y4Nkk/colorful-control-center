mod desktop;
mod fonts;
pub mod model;
mod placement;
mod shortcut;
mod startup;
#[cfg(test)]
mod tests;

use anyhow::{Context, Result, anyhow};
pub use fonts::list as list_fonts;
use model::Preferences;
pub(crate) use placement::save as save_window_placement;
use serde::Serialize;
pub use startup::{create_window, prepare_windows};
use std::{path::PathBuf, sync::Mutex};
use tauri::{AppHandle, Manager};

struct State {
    settings: Option<Preferences>,
    error: Option<String>,
}
pub struct Controller {
    path: PathBuf,
    gate: Mutex<()>,
    state: Mutex<State>,
    acceleration: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub settings: Preferences,
    pub desktop_error: Option<String>,
    pub restart_required: bool,
}

impl Controller {
    pub(crate) fn runtime_acceleration(&self) -> bool {
        self.acceleration
    }

    fn new(path: PathBuf) -> Self {
        let settings = read(&path);
        let acceleration = settings
            .as_ref()
            .map(|s| s.hardware_acceleration)
            .unwrap_or(true);
        Self {
            path,
            gate: Mutex::new(()),
            acceleration,
            state: Mutex::new(State {
                error: settings.as_ref().err().map(|e| format!("{e:#}")),
                settings: settings.ok(),
            }),
        }
    }

    pub fn snapshot(&self) -> Result<Snapshot> {
        let state = self
            .state
            .lock()
            .map_err(|_| anyhow!("软件偏好状态不可用"))?;
        let settings = state
            .settings
            .clone()
            .ok_or_else(|| anyhow!("{}", state.error.as_deref().unwrap_or("软件偏好尚未读取")))?;
        Ok(Snapshot {
            restart_required: settings.hardware_acceleration != self.acceleration,
            settings,
            desktop_error: state.error.clone(),
        })
    }

    pub(crate) fn with_oem_auto_close<T>(
        &self,
        run: impl FnOnce() -> Result<T>,
    ) -> Result<Option<T>> {
        let state = self
            .state
            .lock()
            .map_err(|_| anyhow!("软件偏好状态不可用"))?;
        // 关闭开关的保存响应必须在已经获准的进程操作结束后返回。
        if state
            .settings
            .as_ref()
            .is_some_and(|settings| settings.auto_close_oem)
        {
            return run().map(Some);
        }
        Ok(None)
    }

    fn report(&self, error: Option<String>) {
        if let Ok(mut state) = self.state.lock() {
            state.error = error;
        }
    }

    fn save(
        &self,
        next: Preferences,
        apply: impl Fn(&Preferences) -> Result<()>,
    ) -> Result<Snapshot> {
        next.validate()?;
        let _gate = self
            .gate
            .lock()
            .map_err(|_| anyhow!("软件偏好保存锁不可用"))?;
        let previous = self.snapshot()?.settings;
        anyhow::ensure!(
            read(&self.path)? == previous,
            "软件偏好文件已被更改，请重新读取后再保存"
        );
        let result = apply(&next).and_then(|()| {
            std::fs::create_dir_all(self.path.parent().ok_or_else(|| anyhow!("偏好目录无效"))?)?;
            crate::storage::write(&self.path, &serde_json::to_vec_pretty(&next)?)
                .context("保存软件偏好失败")
        });
        if let Err(error) = result {
            let message = match apply(&previous) {
                Ok(()) => format!("{error:#}"),
                Err(rollback) => {
                    format!("{error:#}；恢复原桌面偏好失败：{rollback:#}。请重试读取或重启应用")
                }
            };
            self.report(Some(message.clone()));
            return Err(anyhow!(message));
        }
        *self
            .state
            .lock()
            .map_err(|_| anyhow!("软件偏好状态不可用"))? = State {
            settings: Some(next),
            error: None,
        };
        self.snapshot()
    }
}

fn read(path: &std::path::Path) -> Result<Preferences> {
    let settings: Preferences = match std::fs::read(path) {
        Ok(data) => serde_json::from_slice(&data).context("软件偏好文件损坏，原文件已保留")?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Preferences::default(),
        Err(error) => return Err(error).context("无法读取软件偏好"),
    };
    settings.validate()?;
    Ok(settings)
}

pub fn plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri::plugin::Builder::new("preferences")
        .setup(|app, _| {
            let controller =
                Controller::new(app.path().app_data_dir()?.join("app-preferences.json"));
            if let Ok(snapshot) = controller.snapshot() {
                log::set_max_level(snapshot.settings.runtime_log_level.filter());
            }
            app.manage(controller);
            app.manage(shortcut::Bindings::default());
            Ok(())
        })
        .build()
}

pub fn get(app: &AppHandle) -> Result<Snapshot> {
    let controller = app.state::<Controller>();
    let _gate = controller
        .gate
        .lock()
        .map_err(|_| anyhow!("软件偏好保存锁不可用"))?;
    let settings = read(&controller.path)?;
    let error = desktop::apply(app, &settings)
        .err()
        .map(|e| format!("{e:#}"));
    *controller
        .state
        .lock()
        .map_err(|_| anyhow!("软件偏好状态不可用"))? = State {
        settings: Some(settings),
        error,
    };
    controller.snapshot()
}

pub fn set(app: &AppHandle, settings: Preferences) -> Result<Snapshot> {
    let result = app
        .state::<Controller>()
        .save(settings, |value| desktop::apply(app, value));
    if result.is_ok() {
        log::info!("软件偏好已保存");
    }
    result
}

pub fn recording(app: &AppHandle, enabled: bool) {
    shortcut::recording(app, enabled);
}
pub fn shortcut_plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    shortcut::plugin()
}
