//! 应用自身信息命令。

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
