//! 自定义标题栏窗口命令；窗口生命周期（关闭到托盘）仍由 tray 模块拥有。

#[tauri::command]
pub(crate) fn window_minimize(window: tauri::WebviewWindow) -> Result<(), String> {
    window.minimize().map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn window_toggle_maximize(window: tauri::WebviewWindow) -> Result<(), String> {
    let maximized = window.is_maximized().map_err(|error| error.to_string())?;
    if maximized {
        window.unmaximize()
    } else {
        window.maximize()
    }
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn window_is_maximized(window: tauri::WebviewWindow) -> Result<bool, String> {
    window.is_maximized().map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn window_close(window: tauri::WebviewWindow) -> Result<(), String> {
    // 必须发出 CloseRequested，让托盘处理器吸收关闭请求。
    window.close().map_err(|error| error.to_string())
}
