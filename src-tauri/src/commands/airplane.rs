use crate::{airplane, hal::Hal};
use tauri::Manager;
#[tauri::command]
pub(crate) async fn read_airplane(app: tauri::AppHandle) -> Result<airplane::Profile, String> {
    crate::shutdown::blocking(move || {
        app.state::<airplane::Controller>()
            .read(&app.state::<Hal>())
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
pub(crate) async fn apply_airplane(
    app: tauri::AppHandle,
    request: airplane::Request,
) -> Result<(), String> {
    crate::shutdown::blocking(move || {
        app.state::<airplane::Controller>()
            .apply(&app.state::<Hal>(), &request)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
pub(crate) async fn restore_airplane(
    app: tauri::AppHandle,
    expected_revision: String,
) -> Result<(), String> {
    crate::shutdown::blocking(move || {
        app.state::<airplane::Controller>()
            .restore(&app.state::<Hal>(), &expected_revision)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}
