//! 原厂供电模式命令只调度领域控制器，目标和恢复由该领域拥有。
use crate::{display, gpu_power, gpuctl::GpuController, hal::Hal, powerctl::mode::supply};
use tauri::{AppHandle, Manager};

#[tauri::command]
pub(crate) async fn read_oem_supply_mode(app: AppHandle) -> Result<supply::Status, String> {
    crate::shutdown::blocking(move || {
        app.state::<supply::Controller>().read(
            &app.state::<Hal>(),
            &app.state::<GpuController>(),
            &app.state::<gpu_power::Controller>(),
        )
    })
    .await
    .map_err(|error| format!("供电模式读取任务失败：{error}"))?
    .map_err(|error| format!("{error:#}"))
}
#[tauri::command]
pub(crate) async fn enable_oem_supply_mode(
    app: AppHandle,
    expected_revision: String,
) -> Result<(), String> {
    crate::shutdown::blocking(move || {
        app.state::<supply::Controller>().enable(
            &app.state::<Hal>(),
            &app.state::<GpuController>(),
            &app.state::<gpu_power::Controller>(),
            &app.state::<display::Controller>(),
            &expected_revision,
        )
    })
    .await
    .map_err(|error| format!("供电模式启用任务失败：{error}"))?
    .map_err(|error| format!("{error:#}"))
}
#[tauri::command]
pub(crate) async fn resume_oem_supply_mode(
    app: AppHandle,
    expected_revision: String,
) -> Result<(), String> {
    crate::shutdown::blocking(move || {
        app.state::<supply::Controller>().resume(
            &app.state::<Hal>(),
            &app.state::<GpuController>(),
            &app.state::<gpu_power::Controller>(),
            &app.state::<display::Controller>(),
            &expected_revision,
        )
    })
    .await
    .map_err(|error| format!("供电模式继续任务失败：{error}"))?
    .map_err(|error| format!("{error:#}"))
}
#[tauri::command]
pub(crate) fn pause_oem_supply_mode(
    controller: tauri::State<'_, supply::Controller>,
) -> Result<(), String> {
    controller.pause().map_err(|error| format!("{error:#}"))
}
#[tauri::command]
pub(crate) async fn restore_oem_supply_mode(
    app: AppHandle,
    expected_revision: String,
) -> Result<(), String> {
    crate::shutdown::blocking(move || {
        app.state::<supply::Controller>().restore(
            &app.state::<Hal>(),
            &app.state::<GpuController>(),
            &app.state::<gpu_power::Controller>(),
            &app.state::<display::Controller>(),
            &expected_revision,
        )
    })
    .await
    .map_err(|error| format!("供电模式恢复任务失败：{error}"))?
    .map_err(|error| format!("{error:#}"))
}
