//! 自动档案命令仅调度唯一策略及应用Controller。
use crate::{
    hal::Hal,
    profiles::{
        self,
        policy::{Controller, Record, Request, View},
    },
};
use tauri::{AppHandle, Manager};

#[tauri::command]
pub(crate) async fn read_power_profile_policy(app: AppHandle) -> Result<View, String> {
    tauri::async_runtime::spawn_blocking(move || app.state::<Controller>().read())
        .await
        .map_err(|error| format!("自动规则读取任务失败：{error}"))?
        .map_err(|error| format!("{error:#}"))
}
#[tauri::command]
pub(crate) async fn save_power_profile_policy(
    app: AppHandle,
    request: Request,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let hal = app.state::<Hal>();
        let gpu = app.state::<crate::gpuctl::GpuController>();
        let power = app.state::<crate::gpu_power::Controller>();
        let session =
            crate::powerctl::mode::links::Context::native(&hal, &gpu, &power).session()?;
        app.state::<Controller>()
            .save(&session.context, &profiles::profiles_dir(&app)?, &request)
    })
    .await
    .map_err(|error| format!("自动规则保存任务失败：{error}"))?
    .map_err(|error| format!("{error:#}"))
}
#[tauri::command]
pub(crate) fn pause_power_profile_policy(
    controller: tauri::State<'_, Controller>,
) -> Result<(), String> {
    controller.pause().map_err(|error| format!("{error:#}"))
}
#[tauri::command]
pub(crate) async fn restore_power_profile_policy(
    app: AppHandle,
    expected: Record,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<Controller>()
            .restore(&app.state::<Hal>(), &expected)
    })
    .await
    .map_err(|error| format!("自动规则恢复任务失败：{error}"))?
    .map_err(|error| format!("{error:#}"))
}
#[tauri::command]
pub(crate) async fn reset_power_profile_policy(
    app: AppHandle,
    expected_revision: String,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<Controller>()
            .reset(&app.state::<Hal>(), &expected_revision)
    })
    .await
    .map_err(|error| format!("自动规则清除任务失败：{error}"))?
    .map_err(|error| format!("{error:#}"))
}

#[tauri::command]
pub(crate) async fn reload_power_profile_policy(app: AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || app.state::<Controller>().reload())
        .await
        .map_err(|error| format!("自动规则重读任务失败：{error}"))?
        .map_err(|error| format!("{error:#}"))
}
#[tauri::command]
pub(crate) async fn read_automatic_profile_application(
    app: AppHandle,
) -> Result<Option<profiles::Application>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<profiles::application::Controller>()
            .automatic_report(&app.state::<crate::display::Controller>())
    })
    .await
    .map_err(|error| format!("自动档案结果读取任务失败：{error}"))?
    .map_err(|error| format!("{error:#}"))
}
