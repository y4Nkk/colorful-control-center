use crate::{hal::Hal, platform::lan};
use tauri::Manager;
#[tauri::command]
pub(crate) async fn read_lan_policy(app: tauri::AppHandle) -> Result<Option<lan::Profile>, String> {
    crate::shutdown::blocking(move || {
        app.state::<lan::Controller>().read(&app.state::<Hal>())
    })
    .await
    .map_err(|error| error.to_string())?
    .map_err(|error| format!("{error:#}"))
}
#[tauri::command]
pub(crate) async fn preview_lan_policy(
    app: tauri::AppHandle,
    request: lan::model::Request,
) -> Result<Vec<lan::model::Change>, String> {
    crate::shutdown::blocking(move || {
        app.state::<lan::Controller>()
            .preview(&app.state::<Hal>(), &request)
    })
    .await
    .map_err(|error| error.to_string())?
    .map_err(|error| format!("{error:#}"))
}
#[tauri::command]
pub(crate) async fn apply_lan_policy(
    app: tauri::AppHandle,
    request: lan::model::Request,
) -> Result<(), String> {
    crate::shutdown::blocking(move || {
        app.state::<lan::Controller>()
            .apply(&app.state::<Hal>(), &request)
    })
    .await
    .map_err(|error| error.to_string())?
    .map_err(|error| format!("{error:#}"))
}
#[tauri::command]
pub(crate) async fn restore_lan_policy(
    app: tauri::AppHandle,
    expected_revision: String,
    device_id: String,
) -> Result<(), String> {
    crate::shutdown::blocking(move || {
        app.state::<lan::Controller>()
            .restore(&app.state::<Hal>(), &expected_revision, &device_id)
    })
    .await
    .map_err(|error| error.to_string())?
    .map_err(|error| format!("{error:#}"))
}
