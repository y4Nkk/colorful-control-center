//! 风扇控制命令层（薄委托 → fanctl；写入语义与钳位见 fanctl 模块头注释）。

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::fanctl;
use crate::hal::Hal;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct FanRestoreRequest {
    target: fanctl::RestoreTarget,
    expected_revision: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpResult {
    pub ok: bool,
    pub message: String,
}

async fn with_fan_lock<T: Send + 'static>(
    app: AppHandle,
    run: impl FnOnce(&Hal, &fanctl::custom::Store) -> T + Send + 'static,
) -> Result<T, String> {
    crate::shutdown::blocking(move || {
        let hal = app.state::<Hal>();
        let _write = hal.begin_write().map_err(|e| e.to_string())?;
        Ok(run(&hal, &app.state::<fanctl::custom::Store>()))
    })
    .await
    .map_err(|e| e.to_string())?
}

fn write_result<T>(operation: &str, result: anyhow::Result<T>) -> OpResult {
    match result {
        Ok(_) => OpResult {
            ok: true,
            message: "已应用".into(),
        },
        Err(error) => {
            let message = format!("{operation}失败，{error:#}");
            log::warn!("[fan] {message}");
            OpResult { ok: false, message }
        }
    }
}

#[tauri::command]
pub(crate) async fn read_fan_profile(app: AppHandle) -> Result<fanctl::custom::View, String> {
    with_fan_lock(app, |hal, store| store.read(hal))
        .await?
        .map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub(crate) async fn save_custom_fan_curves(
    app: AppHandle,
    expected_revision: String,
    changes: fanctl::custom::Curves,
) -> Result<OpResult, String> {
    with_fan_lock(app, move |hal, store| {
        write_result(
            "保存自定义风扇曲线",
            store.save(hal, &expected_revision, changes),
        )
    })
    .await
}

#[tauri::command]
pub(crate) async fn set_fan_mode(app: AppHandle, mode: u8) -> Result<OpResult, String> {
    with_fan_lock(app, move |hal, store| {
        write_result("切换风扇模式", store.select_mode(hal, mode))
    })
    .await
}

#[tauri::command]
pub(crate) async fn set_fan_offset(
    app: AppHandle,
    expected: fanctl::OffsetSource,
    pct: u32,
) -> Result<OpResult, String> {
    with_fan_lock(app, move |hal, _| {
        write_result(
            "调整加速偏移",
            fanctl::set_manual_offset(hal, expected, pct),
        )
    })
    .await
}

#[tauri::command]
pub(crate) async fn restore_fan_settings(
    app: AppHandle,
    request: FanRestoreRequest,
) -> Result<OpResult, String> {
    with_fan_lock(app, move |hal, _| {
        let operation = match request.target {
            fanctl::RestoreTarget::Initial => "恢复初始风扇设置",
            fanctl::RestoreTarget::FirmwareDefault => "恢复固件默认风扇设置",
            fanctl::RestoreTarget::InitialAndFirmwareDefault => "恢复初始及固件默认风扇设置",
        };
        write_result(
            operation,
            fanctl::restore(hal, request.target, &request.expected_revision),
        )
    })
    .await
}
