//! 性能调校命令层（薄委托 → powerctl；钳位与安全语义见该模块头注释）。

use serde::Serialize;
use tauri::{Manager, State};

use crate::hal::Hal;
use crate::powerctl;
use powerctl::mode::{links, selection};

fn mode_context(app: &tauri::AppHandle) -> links::Context<'_> {
    links::Context::native(
        app.state::<Hal>().inner(),
        app.state::<crate::gpuctl::GpuController>().inner(),
        app.state::<crate::gpu_power::Controller>().inner(),
    )
}
fn mode_permit(app: &tauri::AppHandle) -> anyhow::Result<()> {
    let hal = app.state::<Hal>();
    hal.ensure_mode_change_allowed()?;
    crate::fanctl::dust::ensure_manual_allowed(&hal)?;
    crate::hal::oem_owner::ensure_owner_absent()?;
    if app.state::<crate::display::Controller>().trial_pending()? {
        anyhow::bail!("请先保留或恢复显示试用，再执行模式GPU联动");
    }
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpResult {
    pub ok: bool,
    pub message: String,
}

fn to_result<T>(result: anyhow::Result<T>, success: &str) -> Result<OpResult, String> {
    match result {
        Ok(_) => Ok(OpResult {
            ok: true,
            message: success.into(),
        }),
        Err(err) => Ok(OpResult {
            ok: false,
            message: format!("{err:#}"),
        }),
    }
}

#[tauri::command]
pub(crate) async fn read_oem_performance_options(
    app: tauri::AppHandle,
) -> Result<powerctl::mode::boost::Profile, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let hal = app.state::<Hal>();
        let _write = hal.begin_write()?;
        selection::read_options(&mode_context(&app))
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
pub(crate) async fn apply_oem_performance_options(
    app: tauri::AppHandle,
    request: powerctl::mode::boost::Request,
) -> Result<OpResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let hal = app.state::<Hal>();
        let _write = hal.begin_write().map_err(|e| format!("{e:#}"))?;
        to_result(
            powerctl::mode::boost::apply(&hal, &request),
            "性能选项已发送，保存值已回读",
        )
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub(crate) async fn read_oem_power_mode(
    app: tauri::AppHandle,
) -> Result<powerctl::mode::ModeProfile, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let context = mode_context(&app);
        let _gate = context.hal.begin_write()?;
        selection::read(&context)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub(crate) async fn apply_oem_power_mode(
    app: tauri::AppHandle,
    expected: powerctl::mode::Target,
) -> Result<OpResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        to_result(
            selection::apply_preview(mode_context(&app), &expected, &mut || mode_permit(&app)),
            "OEM 模式、GPU及关联策略已应用，保存值已回读",
        )
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub(crate) async fn restore_oem_power_mode(
    app: tauri::AppHandle,
    expected_revision: String,
) -> Result<OpResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        to_result(
            powerctl::mode::restoration::restore(
                mode_context(&app),
                &expected_revision,
                &mut || mode_permit(&app),
            ),
            "初始 OEM 模式及已接管GPU状态已恢复",
        )
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub(crate) fn read_power_profile(hal: State<'_, Hal>) -> Result<powerctl::PowerProfile, String> {
    let _write = hal.begin_write().map_err(|e| format!("{e:#}"))?;
    powerctl::read_profile(&hal).map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn apply_saved_cpu(
    hal: State<'_, Hal>,
    controls: powerctl::CpuControls,
) -> Result<OpResult, String> {
    let _write = hal.begin_write().map_err(|e| format!("{e:#}"))?;
    to_result(
        powerctl::apply_saved(&hal, controls),
        "CPU 设置已保存；即时效果未验证",
    )
}

#[tauri::command]
pub(crate) fn apply_runtime_cpu(
    hal: State<'_, Hal>,
    controls: powerctl::CpuControls,
) -> Result<OpResult, String> {
    let _write = hal.begin_write().map_err(|e| format!("{e:#}"))?;
    to_result(
        powerctl::apply_runtime(&hal, controls),
        "即时 CPU 目标已回读确认",
    )
}

#[tauri::command]
pub(crate) fn restore_power_profile(hal: State<'_, Hal>) -> Result<OpResult, String> {
    let _write = hal.begin_write().map_err(|e| e.to_string())?;
    to_result(powerctl::restore(&hal), "已恢复初始设置")
}
