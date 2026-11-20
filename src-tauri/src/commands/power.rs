//! 性能调校命令层（薄委托 → powerctl；钳位与安全语义见该模块头注释）。

use serde::Serialize;
use tauri::{Manager, State};

use crate::{
    assembly::mode_context,
    display,
    hal::{Hal, oem_owner},
    powerctl,
};
use powerctl::mode::{manual, selection};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpResult {
    pub ok: bool,
    pub message: String,
}

#[derive(Serialize)]
#[serde(untagged)]
pub(crate) enum ModeApplyResult {
    ConfirmFnKey {
        #[serde(rename = "confirmFnKey")]
        confirmation: manual::FnKeyConfirmation,
    },
    Completed(OpResult),
}

fn to_result<T>(result: anyhow::Result<T>, success: &str) -> OpResult {
    match result {
        Ok(_) => OpResult {
            ok: true,
            message: success.into(),
        },
        Err(err) => OpResult {
            ok: false,
            message: format!("{err:#}"),
        },
    }
}

#[tauri::command]
pub(crate) async fn read_oem_performance_options(
    app: tauri::AppHandle,
) -> Result<powerctl::mode::boost::Profile, String> {
    crate::shutdown::blocking(move || {
        let hal = app.state::<Hal>();
        let _write = hal.begin_write()?;
        let mut profile = selection::read_options(&mode_context(&app))?;
        if matches!(profile.current_mode, 2 | 3) {
            profile.blocked_reason = profile.blocked_reason.or_else(oem_owner::blocked_reason);
        }
        Ok(profile)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e: anyhow::Error| format!("{e:#}"))
}
#[tauri::command]
pub(crate) async fn apply_oem_performance_options(
    app: tauri::AppHandle,
    request: powerctl::mode::boost::Request,
) -> Result<OpResult, String> {
    crate::shutdown::blocking(move || {
        let hal = app.state::<Hal>();
        let _write = hal.begin_write()?;
        Ok(to_result(
            oem_owner::ensure_owner_absent()
                .and_then(|()| powerctl::mode::boost::apply(&hal, &request)),
            "性能选项已发送，保存值已回读",
        ))
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e: anyhow::Error| format!("{e:#}"))
}

#[tauri::command]
pub(crate) async fn read_oem_power_mode(
    app: tauri::AppHandle,
) -> Result<powerctl::mode::ModeProfile, String> {
    crate::shutdown::blocking(move || {
        let context = mode_context(&app);
        let _gate = context.hal.begin_write()?;
        let mut profile = selection::read(&context)?;
        if profile.can_restore {
            profile.restore_blocked_reason =
                manual::permit(context.hal, &app.state::<display::Controller>())
                    .err()
                    .map(|error| format!("{error:#}"));
            profile.can_restore = profile.restore_blocked_reason.is_none();
        }
        Ok(profile)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e: anyhow::Error| format!("{e:#}"))
}

#[tauri::command]
pub(crate) async fn apply_oem_power_mode(
    app: tauri::AppHandle,
    expected: powerctl::mode::Target,
    confirmed_fn_key: Option<manual::FnKeyConfirmation>,
) -> Result<ModeApplyResult, String> {
    crate::shutdown::blocking(move || {
        let result = manual::apply(
            mode_context(&app),
            &app.state::<display::Controller>(),
            &expected,
            confirmed_fn_key,
        );
        match result {
            Ok(manual::Apply::ConfirmFnKey(confirmation)) => {
                ModeApplyResult::ConfirmFnKey { confirmation }
            }
            result => ModeApplyResult::Completed(to_result(
                result.map(|_| ()),
                "OEM 模式、GPU及关联策略已应用，保存值已回读",
            )),
        }
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) async fn restore_oem_power_mode(
    app: tauri::AppHandle,
    expected_revision: String,
) -> Result<OpResult, String> {
    crate::shutdown::blocking(move || {
        let context = mode_context(&app);
        let hal = context.hal;
        let display = app.state::<display::Controller>();
        to_result(
            powerctl::mode::restoration::restore(context, &expected_revision, &mut || {
                manual::permit(hal, &display)
            }),
            "初始 OEM 模式及已接管GPU状态已恢复",
        )
    })
    .await
    .map_err(|e| e.to_string())
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
    Ok(to_result(
        powerctl::apply_saved(&hal, controls),
        "CPU 设置已保存；即时效果未验证",
    ))
}

#[tauri::command]
pub(crate) fn apply_runtime_cpu(
    hal: State<'_, Hal>,
    controls: powerctl::CpuControls,
) -> Result<OpResult, String> {
    let _write = hal.begin_write().map_err(|e| format!("{e:#}"))?;
    Ok(to_result(
        powerctl::apply_runtime(&hal, controls),
        "即时 CPU 目标已回读确认",
    ))
}

#[tauri::command]
pub(crate) fn restore_power_profile(hal: State<'_, Hal>) -> Result<OpResult, String> {
    let _write = hal.begin_write().map_err(|e| e.to_string())?;
    Ok(to_result(powerctl::restore(&hal), "已恢复初始设置"))
}
