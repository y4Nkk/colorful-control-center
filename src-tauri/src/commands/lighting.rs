//! 键盘灯光命令层（薄委托 → lighting；编码/事务/回滚语义见该模块头注释）。

use tauri::Manager;

use crate::hal::Hal;
use crate::lighting::{ApplyOutcome, LightingController, LightingProfile, LightingRequest};

#[tauri::command]
pub(crate) async fn read_lighting_options(
    app: tauri::AppHandle,
) -> Result<crate::lighting::options::Profile, String> {
    crate::shutdown::blocking(move || {
        app.state::<crate::lighting::options::Controller>()
            .read(&app.state::<Hal>())
    })
    .await
    .map_err(|e| format!("灯光选项读取任务失败：{e}"))?
    .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
pub(crate) async fn apply_lighting_options(
    app: tauri::AppHandle,
    request: crate::lighting::options::Request,
) -> Result<(), String> {
    crate::shutdown::blocking(move || {
        app.state::<crate::lighting::options::Controller>()
            .apply(&app.state::<Hal>(), &request)
    })
    .await
    .map_err(|e| format!("灯光选项应用任务失败：{e}"))?
    .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
pub(crate) async fn restore_lighting_options(
    app: tauri::AppHandle,
    expected_revision: String,
) -> Result<(), String> {
    crate::shutdown::blocking(move || {
        app.state::<crate::lighting::options::Controller>()
            .restore(&app.state::<Hal>(), &expected_revision)
    })
    .await
    .map_err(|e| format!("灯光选项恢复任务失败：{e}"))?
    .map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub(crate) async fn read_perkey_lighting(
    app: tauri::AppHandle,
) -> Result<crate::lighting::perkey::Profile, String> {
    crate::shutdown::blocking(move || {
        app.state::<crate::lighting::perkey::Controller>()
            .read(&app.state::<Hal>())
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
pub(crate) async fn apply_perkey_lighting(
    app: tauri::AppHandle,
    request: crate::lighting::perkey::Request,
) -> Result<(), String> {
    crate::shutdown::blocking(move || {
        app.state::<crate::lighting::perkey::Controller>()
            .apply(&app.state::<Hal>(), &request)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
pub(crate) async fn restore_perkey_lighting(app: tauri::AppHandle) -> Result<(), String> {
    crate::shutdown::blocking(move || {
        app.state::<crate::lighting::perkey::Controller>()
            .restore(&app.state::<Hal>())
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}

/// 读取键盘灯事实（判型检测 + page2 保存值 + 基线；永不 Err，可用性折叠进 available）。
#[tauri::command]
pub(crate) async fn read_keyboard_lighting(
    app: tauri::AppHandle,
) -> Result<LightingProfile, String> {
    crate::shutdown::blocking(move || {
        app.state::<LightingController>()
            .read_profile(&app.state::<Hal>())
    })
    .await
    .map_err(|e| format!("键盘灯读取任务失败：{e}"))
}

/// 应用已预览的完整灯光草稿；具体校验和事务由灯光域拥有。
#[tauri::command]
pub(crate) async fn apply_keyboard_lighting(
    app: tauri::AppHandle,
    request: LightingRequest,
) -> Result<ApplyOutcome, String> {
    crate::shutdown::blocking(move || {
        app.state::<LightingController>()
            .apply(&app.state::<Hal>(), request)
    })
    .await
    .map_err(|e| format!("键盘灯应用任务失败：{e}"))
}

/// 回放本应用首次写入前的键盘灯基线（原始命令 + page2 事务回写）。
#[tauri::command]
pub(crate) async fn restore_keyboard_lighting(
    app: tauri::AppHandle,
) -> Result<ApplyOutcome, String> {
    crate::shutdown::blocking(move || {
        app.state::<LightingController>()
            .restore(&app.state::<Hal>())
    })
    .await
    .map_err(|e| format!("键盘灯恢复任务失败：{e}"))
}
#[tauri::command]
pub(crate) async fn read_multi_zone_lighting(
    app: tauri::AppHandle,
) -> Result<Option<crate::lighting::multizone::Profile>, String> {
    crate::shutdown::blocking(move || {
        app.state::<crate::lighting::multizone::Controller>()
            .read(&app.state::<Hal>())
    })
    .await
    .map_err(|error| format!("多区键盘读取任务失败：{error}"))?
    .map_err(|error| format!("{error:#}"))
}
#[tauri::command]
pub(crate) async fn apply_multi_zone_lighting(
    app: tauri::AppHandle,
    request: crate::lighting::multizone::Request,
) -> Result<(), String> {
    crate::shutdown::blocking(move || {
        app.state::<crate::lighting::multizone::Controller>()
            .apply(&app.state::<Hal>(), &request)
    })
    .await
    .map_err(|error| format!("多区键盘设置任务失败：{error}"))?
    .map_err(|error| format!("{error:#}"))
}
#[tauri::command]
pub(crate) async fn restore_multi_zone_lighting(
    app: tauri::AppHandle,
    expected_revision: String,
) -> Result<(), String> {
    crate::shutdown::blocking(move || {
        app.state::<crate::lighting::multizone::Controller>()
            .restore(&app.state::<Hal>(), &expected_revision)
    })
    .await
    .map_err(|error| format!("多区键盘恢复任务失败：{error}"))?
    .map_err(|error| format!("{error:#}"))
}
