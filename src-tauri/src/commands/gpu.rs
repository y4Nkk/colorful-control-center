//! GPU 频率偏移命令层（薄委托 → gpuctl；事务/钳位/回滚语义见该模块头注释）。
//!
//! 非整数入参由 serde 在反序列化阶段直接拒绝（Option<i32>），
//! 错误原文返回前端；越界与只读域在 gpuctl 事务校验中拒绝并带中文原因。

use tauri::State;

use crate::gpuctl::{ApplyOutcome, GpuController, GpuProfile};
use crate::hal::{
    Hal,
    authorization::{self, AdministrativeAccess},
};

fn check_write_privilege() -> Result<(), String> {
    match authorization::administrative_access() {
        Ok(AdministrativeAccess::Elevated) => Ok(()),
        Ok(AdministrativeAccess::CanElevate) => Err("调整或恢复 GPU 偏移需要管理员权限。".into()),
        Ok(AdministrativeAccess::Standard) => Err(authorization::SAME_USER_ADMIN_REQUIRED.into()),
        Err(error) => Err(format!("无法确认 GPU 调校所需的管理员权限：{error:#}")),
    }
}

/// 读取 GPU 偏移事实（可用性折叠进 available/unavailableReason，不返回 Err）。
#[tauri::command]
pub(crate) fn read_gpu_profile(
    hal: State<'_, Hal>,
    gpu: State<'_, GpuController>,
) -> Result<GpuProfile, String> {
    let _write = hal.begin_write().map_err(|e| format!("{e:#}"))?;
    let mut profile = gpu.read_profile();
    profile.write_blocked_reason = check_write_privilege().err();
    Ok(profile)
}

/// 事务式应用核心/显存偏移（MHz；None = 该域保持不动）。
#[tauri::command]
pub(crate) fn apply_gpu_clock_offsets(
    hal: State<'_, Hal>,
    gpu: State<'_, GpuController>,
    core_mhz: Option<i32>,
    mem_mhz: Option<i32>,
) -> Result<ApplyOutcome, String> {
    check_write_privilege()?;
    let _write = hal.begin_write().map_err(|e| format!("{e:#}"))?;
    Ok(gpu.apply_offsets(core_mhz, mem_mhz))
}

/// 恢复应用首次写入前的原始基准偏移。
#[tauri::command]
pub(crate) fn restore_gpu_baseline(
    hal: State<'_, Hal>,
    gpu: State<'_, GpuController>,
) -> Result<ApplyOutcome, String> {
    check_write_privilege()?;
    let _write = hal.begin_write().map_err(|e| format!("{e:#}"))?;
    Ok(gpu.restore_baseline())
}
