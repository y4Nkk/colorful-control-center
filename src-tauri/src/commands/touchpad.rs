use crate::{hal::Hal, touchpad};
use tauri::Manager;

#[tauri::command]
pub(crate) async fn read_touchpad(app: tauri::AppHandle) -> Result<touchpad::Profile, String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<touchpad::Controller>()
            .read(&app.state::<Hal>())
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
pub(crate) async fn apply_touchpad(
    app: tauri::AppHandle,
    request: touchpad::Request,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<touchpad::Controller>()
            .apply(&app.state::<Hal>(), &request)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
pub(crate) async fn restore_touchpad(
    app: tauri::AppHandle,
    expected_revision: String,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<touchpad::Controller>()
            .restore(&app.state::<Hal>(), &expected_revision)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}
