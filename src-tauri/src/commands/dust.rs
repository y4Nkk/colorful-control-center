use crate::{fanctl::dust, hal::Hal};
use tauri::Manager;

#[tauri::command]
pub(crate) async fn read_fan_dust(app: tauri::AppHandle) -> Result<Option<dust::Profile>, String> {
    crate::shutdown::blocking(move || {
        app.state::<dust::Controller>().read(&app.state::<Hal>())
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
pub(crate) async fn start_fan_dust(
    app: tauri::AppHandle,
    expected_revision: String,
) -> Result<(), String> {
    crate::shutdown::blocking(move || {
        app.state::<dust::Controller>()
            .start(&app.state::<Hal>(), &expected_revision)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
pub(crate) async fn apply_fan_dust_schedule(
    app: tauri::AppHandle,
    request: dust::Request,
) -> Result<(), String> {
    crate::shutdown::blocking(move || {
        app.state::<dust::Controller>()
            .apply(&app.state::<Hal>(), &request)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
pub(crate) async fn restore_fan_dust_schedule(
    app: tauri::AppHandle,
    expected_revision: String,
) -> Result<(), String> {
    crate::shutdown::blocking(move || {
        app.state::<dust::Controller>()
            .restore(&app.state::<Hal>(), &expected_revision)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}
