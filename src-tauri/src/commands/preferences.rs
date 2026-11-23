//! 软件偏好命令薄层；全部持久化与桌面事务由 preferences 领域拥有。
use crate::preferences::{self, Snapshot, model::Preferences};
use crate::startup::{RestartMode, request_restart};

#[tauri::command]
pub(crate) async fn get_app_preferences(app: tauri::AppHandle) -> Result<Snapshot, String> {
    crate::shutdown::blocking(move || preferences::get(&app))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub(crate) async fn set_app_preferences(
    app: tauri::AppHandle,
    settings: Preferences,
) -> Result<Snapshot, String> {
    crate::shutdown::blocking(move || preferences::set(&app, settings))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub(crate) async fn list_system_fonts() -> Result<Vec<String>, String> {
    crate::shutdown::blocking(preferences::list_fonts)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn set_shortcut_recording(app: tauri::AppHandle, enabled: bool) {
    preferences::recording(&app, enabled);
}

#[tauri::command]
pub(crate) async fn restart_application() -> Result<(), String> {
    restart(RestartMode::Interface).await
}

#[tauri::command]
pub(crate) async fn restart_as_administrator() -> Result<(), String> {
    restart(RestartMode::Administrator).await
}

async fn restart(mode: RestartMode) -> Result<(), String> {
    // UAC 等待不持有硬件操作的退出准入，交接许可时才请求退出。
    tauri::async_runtime::spawn_blocking(move || request_restart(mode))
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| format!("{error:#}"))
}
