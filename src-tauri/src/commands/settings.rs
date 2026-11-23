//! 设置命令薄层：自启和启动档案由 startup 拥有，档案契约由 profiles 拥有。
use tauri::{AppHandle, State};

#[tauri::command]
pub(crate) fn get_oem_startup_notice(
    notice: State<'_, Option<crate::startup::OemStartupNotice>>,
) -> Option<crate::startup::OemStartupNotice> {
    notice.inner().clone()
}

#[tauri::command]
pub(crate) async fn get_autostart(
    app: AppHandle,
) -> Result<crate::startup::login::Snapshot, String> {
    crate::shutdown::blocking(move || {
        crate::startup::login::read(&app).map_err(|e| format!("{e:#}"))
    })
    .await
    .map_err(|e| format!("登录自启读取失败：{e}"))?
}

#[tauri::command]
pub(crate) async fn set_autostart(
    app: AppHandle,
    expected_revision: String,
    level: crate::startup::login::Level,
    restore: bool,
) -> Result<crate::startup::login::Snapshot, String> {
    crate::shutdown::blocking(move || {
        crate::startup::login::apply(&app, &expected_revision, level, restore)
            .map_err(|e| format!("{e:#}"))
    })
    .await
    .map_err(|e| format!("登录自启保存失败：{e}"))?
}

#[tauri::command]
pub(crate) fn get_startup_profile(
    app: AppHandle,
) -> Result<crate::startup::StartupProfile, String> {
    crate::startup::get_profile(&app).map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub(crate) async fn set_startup_profile(
    app: AppHandle,
    expected: Option<crate::profiles::ProfileFile>,
    expected_mode: Option<crate::powerctl::mode::profile::ApprovalPreview>,
) -> Result<Option<crate::profiles::authorization::Binding>, String> {
    crate::shutdown::blocking(move || {
        crate::startup::set_profile(&app, expected, expected_mode.as_ref())
            .map_err(|error| format!("{error:#}"))
    })
    .await
    .map_err(|error| format!("启动档案保存任务失败：{error}"))?
}

#[tauri::command]
pub(crate) fn export_profile(app: AppHandle, name: String) -> Result<String, String> {
    let file = crate::profiles::load(&app, &name).map_err(|e| format!("{e:#}"))?;
    serde_json::to_string_pretty(&file).map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn preview_profile_import(json: String) -> Result<crate::profiles::ProfileFile, String> {
    let file = serde_json::from_str(&json).map_err(|e| format!("JSON 解析失败：{e}"))?;
    crate::profiles::validate_file(&file).map_err(|e| format!("{e:#}"))?;
    Ok(file)
}
