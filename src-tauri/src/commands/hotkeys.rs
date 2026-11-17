use crate::hotkeys;
use tauri::{AppHandle, Emitter, Manager};

#[tauri::command]
pub(crate) async fn restore_battery_alert_audio(
    app: AppHandle,
    expected_revision: String,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let result = app
            .state::<hotkeys::Controller>()
            .restore_battery_alert_audio(&app, &expected_revision);
        if let Err(error) = app.emit_to("main", "hardware-controls-changed", ()) {
            log::warn!("[hotkeys] 电池音频恢复刷新：{error}");
        }
        result
    })
    .await
    .map_err(|error| error.to_string())?
    .map_err(|error| format!("{error:#}"))
}

#[tauri::command]
pub(crate) async fn reset_invalid_hotkeys(
    app: AppHandle,
    expected_revision: String,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<hotkeys::Controller>()
            .reset_invalid(&app, &expected_revision)
    })
    .await
    .map_err(|error| error.to_string())?
    .map_err(|error| format!("{error:#}"))
}

#[tauri::command]
pub(crate) async fn acknowledge_hotkey_navigation(
    app: AppHandle,
    expected_id: u64,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<hotkeys::Controller>()
            .acknowledge_navigation(&app, expected_id)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub(crate) async fn restore_low_battery_brightness(
    app: AppHandle,
    expected_revision: String,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let result = app
            .state::<hotkeys::Controller>()
            .restore_low_battery_brightness(&app, &expected_revision);
        if let Err(error) = app.emit_to("main", "hardware-controls-changed", ()) {
            log::warn!("[hotkeys] 低电量亮度恢复刷新通知：{error}");
        }
        result
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub(crate) async fn read_hotkey_status(app: AppHandle) -> Result<hotkeys::Status, String> {
    tauri::async_runtime::spawn_blocking(move || app.state::<hotkeys::Controller>().status())
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub(crate) async fn dismiss_battery_cold_warning(
    app: AppHandle,
    expected_id: u64,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<hotkeys::Controller>()
            .dismiss_cold_warning(&app, expected_id)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub(crate) async fn restore_pd_event(
    app: AppHandle,
    expected_revision: String,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let result = app
            .state::<hotkeys::Controller>()
            .restore_pd_event(&app, &expected_revision);
        if let Err(error) = app.emit_to("main", "hardware-controls-changed", ()) {
            log::warn!("[hotkeys] PD恢复刷新通知：{error}");
        }
        result
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub(crate) async fn restore_battery_thresholds(
    app: AppHandle,
    expected_revision: String,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let result = app
            .state::<hotkeys::Controller>()
            .restore_battery_thresholds(&app, &expected_revision);
        if let Err(error) = app.emit_to("main", "hardware-controls-changed", ()) {
            log::warn!("[hotkeys] 电池阈值恢复刷新通知：{error}");
        }
        result
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub(crate) async fn restore_gpu_turbo(
    app: AppHandle,
    expected_revision: String,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let result = app
            .state::<hotkeys::Controller>()
            .restore_gpu_turbo(&app, &expected_revision);
        if let Err(error) = app.emit_to("main", "hardware-controls-changed", ()) {
            log::warn!("[hotkeys] GPU 恢复刷新通知：{error}");
        }
        result
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub(crate) async fn end_panel_hotkey(
    app: AppHandle,
    expected_revision: String,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<hotkeys::Controller>()
            .end_panel_hotkey(&app, &expected_revision)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub(crate) async fn restore_screen_brightness(
    app: AppHandle,
    expected_revision: String,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<hotkeys::Controller>()
            .restore_screen_brightness(&app, &expected_revision)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub(crate) async fn read_hotkeys(app: AppHandle) -> Result<hotkeys::Profile, String> {
    tauri::async_runtime::spawn_blocking(move || app.state::<hotkeys::Controller>().read(&app))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
pub(crate) async fn apply_hotkeys(app: AppHandle, request: hotkeys::Request) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<hotkeys::Controller>().apply(&app, request)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
pub(crate) async fn restore_hotkeys(
    app: AppHandle,
    expected_revision: String,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<hotkeys::Controller>()
            .restore(&app, &expected_revision)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
pub(crate) async fn stop_hotkeys(app: AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || app.state::<hotkeys::Controller>().stop(&app))
        .await
        .map_err(|e| e.to_string())?
        .map(|_| ())
        .map_err(|e| format!("{e:#}"))
}
