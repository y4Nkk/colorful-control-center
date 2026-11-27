//! 电池命令薄层，策略、校验和恢复由battery领域拥有。
use crate::{battery, hal::Hal};
use tauri::Manager;

#[tauri::command]
pub(crate) async fn read_energy_policy(
    app: tauri::AppHandle,
) -> Result<Option<battery::energy::Profile>, String> {
    crate::shutdown::blocking(move || {
        app.state::<battery::Controller>()
            .read_energy(&app.state::<Hal>())
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
pub(crate) async fn apply_energy_policy(
    app: tauri::AppHandle,
    request: battery::energy::Request,
) -> Result<(), String> {
    crate::shutdown::blocking(move || {
        app.state::<battery::Controller>()
            .apply_energy(&app.state::<Hal>(), &request)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
pub(crate) async fn restore_energy_policy(
    app: tauri::AppHandle,
    expected_revision: String,
) -> Result<(), String> {
    crate::shutdown::blocking(move || {
        app.state::<battery::Controller>()
            .restore_energy(&app.state::<Hal>(), &expected_revision)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub(crate) async fn read_battery_profile(
    app: tauri::AppHandle,
) -> Result<battery::Profile, String> {
    crate::shutdown::blocking(move || {
        app.state::<battery::Controller>().read(&app.state::<Hal>())
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
pub(crate) async fn apply_battery_charge(
    app: tauri::AppHandle,
    request: battery::ChargeRequest,
) -> Result<(), String> {
    crate::shutdown::blocking(move || {
        app.state::<battery::Controller>()
            .apply(&app.state::<Hal>(), &request)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}
