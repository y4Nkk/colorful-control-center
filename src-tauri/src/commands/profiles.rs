//! 性能档案命令层（薄委托 → profiles；应用流水线每步独立报告）。

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::hal::Hal;
use crate::profiles::{Application, ProfileMeta, ProfileSettings};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveProfileArgs {
    pub name: String,
    pub settings: ProfileSettings,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpResult {
    pub ok: bool,
    pub message: String,
}

#[tauri::command]
pub(crate) fn save_profile(app: AppHandle, args: SaveProfileArgs) -> Result<OpResult, String> {
    match crate::profiles::save(&app, args.settings, &args.name) {
        Ok(()) => {
            crate::tray::refresh(&app);
            Ok(OpResult {
                ok: true,
                message: "档案已保存".into(),
            })
        }
        Err(err) => Ok(OpResult {
            ok: false,
            message: format!("{err:#}"),
        }),
    }
}

#[tauri::command]
pub(crate) fn list_profiles(app: AppHandle) -> Result<Vec<ProfileMeta>, String> {
    crate::profiles::list(&app).map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub(crate) fn load_profile(
    app: AppHandle,
    name: String,
) -> Result<crate::profiles::ProfileFile, String> {
    crate::profiles::load(&app, &name).map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub(crate) fn delete_profile(app: AppHandle, name: String) -> Result<OpResult, String> {
    match crate::profiles::delete(&app, &name) {
        Ok(unlinked) => {
            crate::tray::refresh(&app);
            Ok(OpResult {
                ok: true,
                message: if unlinked {
                    "档案已删除，并已取消该档案的开机应用"
                } else {
                    "档案已删除"
                }
                .into(),
            })
        }
        Err(err) => Ok(OpResult {
            ok: false,
            message: format!("{err:#}"),
        }),
    }
}

#[tauri::command]
pub(crate) async fn capture_profile_settings(app: AppHandle) -> Result<ProfileSettings, String> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::profiles::capture_current(
            &app.state::<Hal>(),
            &app.state::<crate::gpuctl::GpuController>(),
            crate::profiles::Devices::from_app(&app),
        )
        .map_err(|e| format!("{e:#}"))
    })
    .await
    .map_err(|e| format!("档案捕获任务失败：{e}"))?
}

#[tauri::command]
pub(crate) async fn preview_profile_authorization(
    app: AppHandle,
    settings: ProfileSettings,
) -> Result<Option<crate::powerctl::mode::profile::ApprovalPreview>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let hal = app.state::<Hal>();
        let gpu = app.state::<crate::gpuctl::GpuController>();
        let power = app.state::<crate::gpu_power::Controller>();
        let session =
            crate::powerctl::mode::links::Context::native(&hal, &gpu, &power).session()?;
        let _gate = hal.begin_write()?;
        crate::profiles::preview_authorization(&session.context, &settings)
    })
    .await
    .map_err(|error| format!("档案授权预览任务失败：{error}"))?
    .map_err(|error| format!("{error:#}"))
}

#[tauri::command]
pub(crate) async fn preview_profile_mode(
    app: AppHandle,
    settings: ProfileSettings,
) -> Result<Option<crate::powerctl::mode::Target>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let hal = app.state::<Hal>();
        let gpu = app.state::<crate::gpuctl::GpuController>();
        let power = app.state::<crate::gpu_power::Controller>();
        let session =
            crate::powerctl::mode::links::Context::native(&hal, &gpu, &power).session()?;
        let _gate = hal.begin_write()?;
        crate::profiles::preview_mode(&session.context, &settings)
    })
    .await
    .map_err(|error| format!("档案模式预览任务失败：{error}"))?
    .map_err(|error| format!("{error:#}"))
}

#[tauri::command]
pub(crate) async fn apply_profile(
    app: AppHandle,
    expected: crate::profiles::ProfileFile,
    expected_mode: Option<crate::powerctl::mode::Target>,
    expected_display: Option<crate::display::profile::Review>,
    expected_gpu_power: Option<crate::gpu_power::profile::Review>,
    expected_graphics_mode: Option<crate::graphics_mode::profile::Review>,
) -> Result<Application, String> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::profiles::apply_preview(
            &app,
            &app.state::<Hal>(),
            &app.state::<crate::gpuctl::GpuController>(),
            &expected,
            expected_mode.as_ref(),
            expected_display.as_ref(),
            expected_gpu_power.as_ref(),
            expected_graphics_mode.as_ref(),
        )
        .map_err(|e| format!("{e:#}"))
    })
    .await
    .map_err(|e| format!("档案应用任务失败：{e}"))?
}

#[tauri::command]
pub(crate) async fn preview_profile_display(
    app: AppHandle,
    settings: ProfileSettings,
) -> Result<Option<crate::display::profile::Review>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::profiles::validate(&settings)?;
        let hal = app.state::<Hal>();
        let _gate = hal.begin_write()?;
        settings
            .display
            .as_ref()
            .map(|target| {
                app.state::<crate::display::Controller>()
                    .preview_profile(target, None)
            })
            .transpose()
    })
    .await
    .map_err(|error| format!("显示预览任务失败：{error}"))?
    .map_err(|error: anyhow::Error| format!("{error:#}"))
}
#[tauri::command]
pub(crate) async fn preview_profile_gpu_power(
    app: AppHandle,
    settings: ProfileSettings,
) -> Result<Option<crate::gpu_power::profile::Review>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::profiles::validate(&settings)?;
        let controller = app.state::<crate::gpu_power::Controller>();
        let scope = controller.scope()?;
        let hal = app.state::<Hal>();
        let _gate = hal.begin_write()?;
        settings
            .gpu_power
            .as_ref()
            .map(|target| scope.preview_profile(&hal, target))
            .transpose()
    })
    .await
    .map_err(|error| format!("独显预览任务失败：{error}"))?
    .map_err(|error: anyhow::Error| format!("{error:#}"))
}
#[tauri::command]
pub(crate) async fn read_profile_application(
    app: AppHandle,
) -> Result<Option<Application>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let report = app
            .state::<crate::profiles::application::Controller>()
            .read(&app.state::<crate::display::Controller>())?;
        Ok(report)
    })
    .await
    .map_err(|error| format!("档案状态任务失败：{error}"))?
    .map_err(|error: anyhow::Error| format!("{error:#}"))
}
#[tauri::command]
pub(crate) async fn keep_profile_display(app: AppHandle, id: u64) -> Result<Application, String> {
    tauri::async_runtime::spawn_blocking(move || crate::profiles::application::keep(&app, id))
        .await
        .map_err(|error| format!("档案确认任务失败：{error}"))?
        .map_err(|error| format!("{error:#}"))
}
#[tauri::command]
pub(crate) async fn cancel_profile_application(
    app: AppHandle,
    id: u64,
) -> Result<Application, String> {
    tauri::async_runtime::spawn_blocking(move || crate::profiles::application::cancel(&app, id))
        .await
        .map_err(|error| format!("档案恢复任务失败：{error}"))?
        .map_err(|error| format!("{error:#}"))
}
#[tauri::command]
pub(crate) async fn preview_profile_graphics_mode(
    app: AppHandle,
    settings: ProfileSettings,
) -> Result<Option<crate::graphics_mode::profile::Review>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::profiles::validate(&settings)?;
        let controller = app.state::<crate::graphics_mode::Controller>();
        let scope = controller.scope()?;
        let hal = app.state::<Hal>();
        let _gate = hal.begin_write()?;
        settings
            .graphics_mode
            .as_ref()
            .map(|target| scope.preview_profile(&hal, target))
            .transpose()
    })
    .await
    .map_err(|error| format!("显卡模式预览任务失败：{error}"))?
    .map_err(|error: anyhow::Error| format!("{error:#}"))
}
