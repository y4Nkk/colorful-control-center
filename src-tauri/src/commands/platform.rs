use crate::{hal::Hal, platform};
use tauri::Manager;

#[tauri::command]
pub(crate) async fn read_platform_power(
    app: tauri::AppHandle,
) -> Result<platform::Profile, String> {
    crate::shutdown::blocking(move || {
        app.state::<platform::Controller>()
            .read(&app.state::<Hal>())
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
pub(crate) async fn apply_pd_level(
    app: tauri::AppHandle,
    request: platform::Request,
) -> Result<(), String> {
    crate::shutdown::blocking(move || {
        app.state::<platform::Controller>()
            .apply(&app.state::<Hal>(), &request)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
pub(crate) async fn restore_pd_level(
    app: tauri::AppHandle,
    expected_revision: String,
) -> Result<(), String> {
    crate::shutdown::blocking(move || {
        app.state::<platform::Controller>()
            .restore(&app.state::<Hal>(), &expected_revision)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}
