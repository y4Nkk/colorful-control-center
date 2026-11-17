use crate::{hal::Hal, microphone};
use tauri::Manager;

#[tauri::command]
pub(crate) async fn read_microphones(app: tauri::AppHandle) -> Result<microphone::Profile, String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<microphone::Controller>()
            .read(&app.state::<Hal>())
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
pub(crate) async fn apply_microphones(
    app: tauri::AppHandle,
    request: microphone::Request,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<microphone::Controller>()
            .apply(&app.state::<Hal>(), &request)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
pub(crate) async fn restore_microphones(
    app: tauri::AppHandle,
    expected_revision: String,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<microphone::Controller>()
            .restore(&app.state::<Hal>(), &expected_revision)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}
