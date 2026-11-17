//! 软件偏好命令薄层；全部持久化与桌面事务由 preferences 领域拥有。
use crate::preferences::{self, Snapshot, model::Preferences};

#[tauri::command]
pub(crate) async fn get_app_preferences(app: tauri::AppHandle) -> Result<Snapshot, String> {
    tauri::async_runtime::spawn_blocking(move || preferences::get(&app))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub(crate) async fn set_app_preferences(
    app: tauri::AppHandle,
    settings: Preferences,
) -> Result<Snapshot, String> {
    tauri::async_runtime::spawn_blocking(move || preferences::set(&app, settings))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub(crate) async fn list_system_fonts() -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(preferences::list_fonts)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn set_shortcut_recording(app: tauri::AppHandle, enabled: bool) {
    preferences::recording(&app, enabled);
}

#[tauri::command]
pub(crate) fn restart_application(app: tauri::AppHandle) -> Result<(), String> {
    preferences::restart(&app).map_err(|error| format!("{error:#}"))
}
