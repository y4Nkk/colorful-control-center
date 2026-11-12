//! 硬件监控命令层（薄委托 → hal::Hal）。快照结构见 hal::HardwareSnapshot。

use tauri::State;

use crate::hal::{HardwareSnapshot, Hal};

/// 一次性采样（前端初始渲染用；持续更新走 hardware-snapshot 事件）。
#[tauri::command]
pub(crate) fn read_hardware_snapshot(hal: State<'_, Hal>) -> HardwareSnapshot {
    hal.snapshot()
}
