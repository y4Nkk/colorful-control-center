use crate::{hal::Hal, keyboard};
use tauri::Manager;

#[tauri::command]
pub(crate) async fn read_keyboard_settings(
    app: tauri::AppHandle,
) -> Result<keyboard::Profile, String> {
    crate::shutdown::blocking(move || {
        app.state::<keyboard::Controller>()
            .read(&app.state::<Hal>())
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
pub(crate) async fn apply_keyboard_settings(
    app: tauri::AppHandle,
    request: keyboard::Request,
) -> Result<(), String> {
    crate::shutdown::blocking(move || {
        app.state::<keyboard::Controller>()
            .apply(&app.state::<Hal>(), &request)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}
