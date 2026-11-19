use crate::{hal::Hal, platform::usb};
use tauri::{AppHandle, Manager};
#[tauri::command]
pub(crate) async fn read_usb_policy(app: AppHandle) -> Result<Option<usb::Profile>, String> {
    crate::shutdown::blocking(move || {
        app.state::<usb::Controller>().read(&app.state::<Hal>())
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
pub(crate) async fn apply_usb_policy(app: AppHandle, request: usb::Request) -> Result<(), String> {
    crate::shutdown::blocking(move || {
        app.state::<usb::Controller>()
            .apply(&app.state::<Hal>(), &request)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
pub(crate) async fn restore_usb_policy(
    app: AppHandle,
    expected_revision: String,
) -> Result<(), String> {
    crate::shutdown::blocking(move || {
        app.state::<usb::Controller>()
            .restore(&app.state::<Hal>(), &expected_revision)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}
