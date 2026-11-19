use crate::{battery, hal::Hal};
use tauri::Manager;

#[tauri::command]
pub(crate) async fn read_battery_calibration(
    app: tauri::AppHandle,
) -> Result<Option<battery::calibration::Profile>, String> {
    crate::shutdown::blocking(move || {
        app.state::<battery::Controller>()
            .read_calibration(&app.state::<Hal>())
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
pub(crate) async fn start_battery_calibration(
    app: tauri::AppHandle,
    expected_revision: String,
) -> Result<(), String> {
    crate::shutdown::blocking(move || {
        app.state::<battery::Controller>()
            .start_calibration(&app.state::<Hal>(), &expected_revision)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
pub(crate) async fn stop_battery_calibration(
    app: tauri::AppHandle,
    expected_revision: String,
) -> Result<(), String> {
    crate::shutdown::blocking(move || {
        app.state::<battery::Controller>()
            .stop_calibration(&app.state::<Hal>(), &expected_revision)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
pub(crate) async fn restore_battery_calibration(
    app: tauri::AppHandle,
    expected_revision: String,
) -> Result<(), String> {
    crate::shutdown::blocking(move || {
        app.state::<battery::Controller>()
            .restore_calibration(&app.state::<Hal>(), &expected_revision)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}
