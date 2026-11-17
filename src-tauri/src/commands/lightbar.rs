use crate::{hal::Hal, lighting::lightbar};
use tauri::{AppHandle, Manager};

#[tauri::command]
pub(crate) async fn read_light_bar(app: AppHandle) -> Result<Option<lightbar::Profile>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<lightbar::Controller>()
            .read(&app.state::<Hal>())
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
pub(crate) async fn apply_light_bar(
    app: AppHandle,
    request: lightbar::Request,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<lightbar::Controller>()
            .apply(&app.state::<Hal>(), &request)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
pub(crate) async fn restore_light_bar(
    app: AppHandle,
    expected_revision: String,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<lightbar::Controller>()
            .restore(&app.state::<Hal>(), &expected_revision)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}
