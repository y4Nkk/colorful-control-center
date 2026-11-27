//! 显示/独显命令只调度；设备身份、预检及恢复归领域所有。
use crate::{display, gpu_power, graphics_mode, hal::Hal};
use tauri::Manager;
#[tauri::command]
pub(crate) async fn read_graphics_mode(
    app: tauri::AppHandle,
) -> Result<graphics_mode::Profile, String> {
    crate::shutdown::blocking(move || {
        app.state::<graphics_mode::Controller>()
            .read(&app.state::<Hal>())
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
pub(crate) async fn apply_graphics_mode(
    app: tauri::AppHandle,
    request: graphics_mode::Request,
) -> Result<(), String> {
    crate::shutdown::blocking(move || {
        app.state::<graphics_mode::Controller>().apply(
            &app.state::<Hal>(),
            &app.state::<display::Controller>(),
            &request,
        )
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
pub(crate) async fn reboot_for_graphics_mode(
    app: tauri::AppHandle,
    expected_revision: String,
) -> Result<(), String> {
    crate::shutdown::blocking(move || {
        app.state::<graphics_mode::Controller>().reboot(
            &app.state::<Hal>(),
            &app.state::<display::Controller>(),
            &expected_revision,
        )
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
pub(crate) async fn read_display_modes(app: tauri::AppHandle) -> Result<display::Profile, String> {
    crate::shutdown::blocking(move || {
        let mut profile = app.state::<display::Controller>().read()?;
        if profile.pending.is_none()
            && (profile.rotation.is_some()
                || profile
                    .displays
                    .iter()
                    .any(|display| display.frequencies.len() > 1))
        {
            profile.blocked_reason = crate::hal::oem_owner::blocked_reason();
        }
        Ok(profile)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e: anyhow::Error| format!("{e:#}"))
}
#[tauri::command]
pub(crate) async fn read_automatic_display_trial(
    app: tauri::AppHandle,
) -> Result<Option<display::Pending>, String> {
    crate::shutdown::blocking(move || {
        let Some(display) = app.try_state::<display::Controller>() else {
            return Ok(None);
        };
        display.automatic_pending()
    })
    .await
    .map_err(|error| error.to_string())?
    .map_err(|error| format!("{error:#}"))
}
#[tauri::command]
pub(crate) async fn read_automatic_refresh(
    app: tauri::AppHandle,
) -> Result<display::automatic::Status, String> {
    crate::shutdown::blocking(move || {
        app.state::<display::automatic::Controller>()
            .read(&app.state::<Hal>(), &app.state::<display::Controller>())
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
pub(crate) async fn enable_automatic_refresh(
    app: tauri::AppHandle,
    expected_revision: String,
) -> Result<(), String> {
    crate::shutdown::blocking(move || {
        app.state::<display::automatic::Controller>().enable(
            &app.state::<Hal>(),
            &app.state::<display::Controller>(),
            &expected_revision,
        )
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
pub(crate) async fn restore_automatic_refresh(
    app: tauri::AppHandle,
    expected_revision: String,
) -> Result<(), String> {
    crate::shutdown::blocking(move || {
        app.state::<display::automatic::Controller>().restore(
            &app.state::<Hal>(),
            &app.state::<display::Controller>(),
            &expected_revision,
        )
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
pub(crate) async fn apply_display_refresh(
    app: tauri::AppHandle,
    expected: display::Mode,
    frequency: u32,
) -> Result<(), String> {
    crate::shutdown::blocking(move || {
        app.state::<display::Controller>()
            .apply_runtime(&app.state::<Hal>(), &expected, frequency)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
pub(crate) async fn apply_display_rotation(
    app: tauri::AppHandle,
    expected: display::Mode,
) -> Result<(), String> {
    crate::shutdown::blocking(move || {
        app.state::<display::Controller>()
            .apply_rotation_runtime(&app.state::<Hal>(), &expected)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
pub(crate) async fn keep_display_mode(
    app: tauri::AppHandle,
    trial_id: u64,
    expected: display::Mode,
) -> Result<(), String> {
    crate::shutdown::blocking(move || {
        let display = app.state::<display::Controller>();
        if display.automatic_trial_id()? == Some(trial_id) {
            app.state::<display::automatic::Controller>().keep(
                &app.state::<Hal>(),
                &display,
                trial_id,
                &expected,
            )
        } else {
            display.keep_runtime(&app.state::<Hal>(), trial_id, &expected)
        }
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
pub(crate) async fn cancel_display_trial(
    app: tauri::AppHandle,
    trial_id: u64,
) -> Result<(), String> {
    crate::shutdown::blocking(move || {
        app.state::<display::Controller>()
            .cancel_runtime(&app.state::<Hal>(), trial_id)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
pub(crate) async fn read_gpu_power(app: tauri::AppHandle) -> Result<gpu_power::Profile, String> {
    crate::shutdown::blocking(move || {
        app.state::<gpu_power::Controller>().read_runtime(
            &app.state::<Hal>(),
            &app.state::<crate::gpuctl::GpuController>(),
        )
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
pub(crate) async fn apply_gpu_power(
    app: tauri::AppHandle,
    expected: gpu_power::Device,
    enabled: bool,
) -> Result<(), String> {
    crate::shutdown::blocking(move || {
        let hal = app.state::<Hal>();
        app.state::<gpu_power::Controller>().apply_runtime(
            &hal,
            &app.state::<crate::gpuctl::GpuController>(),
            &app.state::<display::Controller>(),
            &expected,
            enabled,
        )
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}
