//! 硬件监控命令层（薄委托 → hal::Hal）。快照结构见 hal::HardwareSnapshot。

use tauri::State;

use crate::hal::{Hal, HardwareSnapshot};

/// 一次性采样（前端初始渲染用；持续更新走 hardware-snapshot 事件）。
#[tauri::command]
pub(crate) fn read_hardware_snapshot(hal: State<'_, Hal>) -> HardwareSnapshot {
    hal.display_snapshot()
}

#[tauri::command]
pub(crate) async fn read_system_overview() -> Result<crate::hal::overview::SystemOverview, String> {
    tauri::async_runtime::spawn_blocking(crate::hal::overview::read)
        .await
        .map_err(|error| format!("系统概览读取失败：{error}"))
}
