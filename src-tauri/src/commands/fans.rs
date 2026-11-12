//! 风扇控制命令层（薄委托 → fanctl；写入语义与钳位见 fanctl 模块头注释）。

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::fanctl::{self, FanProfile};
use crate::hal::Hal;

/// 单风扇可调点（None 字段 = 保持现值）。
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurvePointsDto {
    pub t2: f64,
    pub d2_pct: f64,
    pub t3: f64,
    pub d3_pct: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpResult {
    pub ok: bool,
    pub message: String,
}

fn to_result<T>(result: anyhow::Result<T>) -> Result<OpResult, String> {
    match result {
        Ok(_) => Ok(OpResult { ok: true, message: "已应用".into() }),
        Err(err) => Ok(OpResult { ok: false, message: err.to_string() }),
    }
}

#[tauri::command]
pub(crate) fn read_fan_profile(hal: State<'_, Hal>) -> Result<FanProfile, String> {
    fanctl::read_profile(&hal).map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn apply_fan_curve(
    hal: State<'_, Hal>,
    cpu: Option<CurvePointsDto>,
    gpu1: Option<CurvePointsDto>,
    gpu2: Option<CurvePointsDto>,
) -> Result<OpResult, String> {
    let to_points = |dto: Option<CurvePointsDto>| {
        dto.map(|p| ccc_core::fan::OemCurvePoints {
            t2: p.t2,
            d2: p.d2_pct,
            t3: p.t3,
            d3: p.d3_pct,
        })
    };
    to_result(fanctl::apply_curve(&hal, to_points(cpu), to_points(gpu1), to_points(gpu2)))
}

#[tauri::command]
pub(crate) fn set_fan_mode(hal: State<'_, Hal>, mode: u8) -> Result<OpResult, String> {
    to_result(fanctl::set_mode(&hal, mode))
}

#[tauri::command]
pub(crate) fn set_fan_offset(hal: State<'_, Hal>, pct: u32) -> Result<OpResult, String> {
    to_result(fanctl::set_offset(&hal, pct))
}

#[tauri::command]
pub(crate) fn restore_fan_default(hal: State<'_, Hal>) -> Result<OpResult, String> {
    to_result(fanctl::restore_default(&hal))
}
