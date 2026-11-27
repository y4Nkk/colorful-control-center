//! 应用信息和启动生命周期命令。

use crate::hal::authorization::{AdministrativeAccess, administrative_access};
use crate::startup::{RestartMode, request_restart};
use tauri::Manager;
use tauri_plugin_opener::OpenerExt;

/// 前端启动冒烟命令：证明 IPC 全链路可用，并给"关于"信息提供事实来源。
#[tauri::command]
pub(crate) fn get_backend_info(app: tauri::AppHandle) -> BackendInfo {
    let package = app.package_info();
    BackendInfo {
        name: package.name.to_string(),
        version: package.version.to_string(),
    }
}

#[derive(serde::Serialize)]
pub(crate) struct BackendInfo {
    pub name: String,
    pub version: String,
}

#[tauri::command]
pub(crate) fn read_administrator_access() -> Result<AdministrativeAccess, String> {
    administrative_access().map_err(|error| format!("{error:#}"))
}

#[tauri::command]
pub(crate) async fn restart_application() -> Result<(), String> {
    restart(RestartMode::Interface).await
}

#[tauri::command]
pub(crate) async fn restart_as_administrator() -> Result<(), String> {
    restart(RestartMode::Administrator).await
}

async fn restart(mode: RestartMode) -> Result<(), String> {
    // UAC 等待不持有硬件操作的退出准入，交接许可时才请求退出。
    tauri::async_runtime::spawn_blocking(move || request_restart(mode))
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| format!("{error:#}"))
}

#[tauri::command]
pub(crate) fn read_application_preparation(app: tauri::AppHandle) -> crate::startup::Snapshot {
    app.state::<crate::startup::Preparation>().snapshot()
}

#[tauri::command]
pub(crate) fn open_startup_logs(app: tauri::AppHandle) -> Result<(), String> {
    let path = app
        .path()
        .app_log_dir()
        .map_err(|error| error.to_string())?;
    app.opener()
        .open_path(path.to_string_lossy().into_owned(), None::<String>)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn exit_application() {
    crate::shutdown::request();
}
