//! 性能调校命令层（薄委托 → powerctl；钳位与安全语义见该模块头注释）。

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::hal::Hal;
use crate::powerctl;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpResult {
    pub ok: bool,
    pub message: String,
}

fn to_result<T>(result: anyhow::Result<T>, success: &str) -> Result<OpResult, String> {
    match result {
        Ok(_) => Ok(OpResult { ok: true, message: success.into() }),
        Err(err) => Ok(OpResult { ok: false, message: err.to_string() }),
    }
}

#[tauri::command]
pub(crate) fn read_power_profile(hal: State<'_, Hal>) -> Result<powerctl::PowerProfile, String> {
    powerctl::read_profile(&hal).map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn apply_pl_persist(
    hal: State<'_, Hal>,
    pl1_w: Option<f64>,
    pl2_w: Option<f64>,
) -> Result<OpResult, String> {
    to_result(
        powerctl::apply_pl_persist(&hal, pl1_w, pl2_w),
        "功耗墙已写入持久化通道（page6）",
    )
}

#[tauri::command]
pub(crate) fn apply_voltage_persist(
    hal: State<'_, Hal>,
    core_mv: Option<i32>,
    gt_mv: Option<i32>,
) -> Result<OpResult, String> {
    to_result(
        powerctl::apply_voltage_persist(&hal, core_mv, gt_mv),
        "电压偏移已写入持久化通道（page6）",
    )
}

#[tauri::command]
pub(crate) fn apply_pl_runtime(
    hal: State<'_, Hal>,
    pl1_w: Option<f64>,
    pl2_w: Option<f64>,
) -> Result<OpResult, String> {
    to_result(powerctl::apply_pl_runtime(&hal, pl1_w, pl2_w), "功耗墙已即时生效（MSR）")
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoltageRuntimeArgs {
    /// 0=CPU core, 1=GPU, 2=cache
    pub plane: u64,
    pub mv: i32,
}

#[tauri::command]
pub(crate) fn apply_voltage_runtime(
    hal: State<'_, Hal>,
    args: VoltageRuntimeArgs,
) -> Result<OpResult, String> {
    match powerctl::apply_voltage_runtime(&hal, args.plane, args.mv) {
        Ok(after) => Ok(OpResult {
            ok: true,
            message: format!("已生效，回读 {after:+.1} mV"),
        }),
        Err(err) => Ok(OpResult { ok: false, message: err.to_string() }),
    }
}
