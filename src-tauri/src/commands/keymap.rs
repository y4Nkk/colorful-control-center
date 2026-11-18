use crate::{hal::Hal, keymap};
use tauri::Manager;
#[tauri::command]
pub(crate) fn preview_keymap_import(json: String) -> Result<keymap::file::File, String> {
    keymap::file::preview(&json).map_err(|e| format!("{e:#}"))
}
#[tauri::command]
pub(crate) async fn retry_keymap_programs(
    app: tauri::AppHandle,
    expected_revision: String,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<keymap::Controller>()
            .retry_programs(&app.state::<Hal>(), &expected_revision)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
pub(crate) async fn read_keymap(app: tauri::AppHandle) -> Result<Option<keymap::Profile>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<keymap::Controller>().read(&app.state::<Hal>())
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
pub(crate) async fn apply_keymap(
    app: tauri::AppHandle,
    request: keymap::Request,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<keymap::Controller>()
            .apply(&app.state::<Hal>(), &request)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
pub(crate) async fn restore_keymap(
    app: tauri::AppHandle,
    expected_revision: String,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<keymap::Controller>()
            .restore(&app.state::<Hal>(), &expected_revision)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub(crate) async fn start_keymap_recording(
    app: tauri::AppHandle,
) -> Result<keymap::recording::Snapshot, String> {
    if !app
        .get_webview_window("main")
        .is_some_and(|window| window.is_focused().unwrap_or(false))
    {
        return Err("请先聚焦主窗口，再开始录制".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<keymap::Controller>().start_recording()
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
pub(crate) async fn read_keymap_recording(
    app: tauri::AppHandle,
) -> Result<Option<keymap::recording::Snapshot>, String> {
    tauri::async_runtime::spawn_blocking(move || app.state::<keymap::Controller>().read_recording())
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
pub(crate) async fn stop_keymap_recording(
    app: tauri::AppHandle,
) -> Result<Option<keymap::recording::Snapshot>, String> {
    tauri::async_runtime::spawn_blocking(move || app.state::<keymap::Controller>().stop_recording())
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("{e:#}"))
}
