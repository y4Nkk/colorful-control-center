//! 硬件监控与系统概览命令层。

use tauri::{Manager, State, WebviewWindow};

use crate::hal::{
    Hal,
    nvidia::GpuDetails,
    overview::{self, Capacity, SystemInventory},
};
use crate::monitoring::Controller;

#[tauri::command]
pub(crate) fn update_hardware_monitoring(
    window: WebviewWindow,
    controller: State<'_, Controller>,
    active: bool,
) -> Result<(), String> {
    if window.label() != "main" {
        return Err("只有主窗口可以订阅硬件监控".into());
    }
    controller.set_active(active);
    Ok(())
}

#[tauri::command]
pub(crate) fn read_memory_capacity() -> Result<Capacity, String> {
    overview::read_memory().map_err(|error| format!("内存容量读取失败：{error}"))
}

#[tauri::command]
pub(crate) async fn read_system_inventory(
    refresh_volume_metadata: bool,
) -> Result<SystemInventory, String> {
    crate::shutdown::blocking(move || overview::read_inventory(refresh_volume_metadata))
        .await
        .and_then(|result| result)
        .map_err(|error| format!("系统设备资料读取失败：{error}"))
}

#[tauri::command]
pub(crate) async fn read_gpu_details(
    window: WebviewWindow,
    connection_id: u32,
) -> Result<Option<GpuDetails>, String> {
    if window.label() != "main" {
        return Err("只有主窗口可以读取 GPU 详情".into());
    }
    crate::shutdown::blocking(move || {
        if !window.is_visible()? || window.is_minimized()? {
            return Ok(None);
        }
        window.state::<Hal>().read_gpu_details(connection_id)
    })
    .await
    .and_then(|result| result)
    .map_err(|error| format!("GPU 详情读取失败：{error:#}"))
}
