//! 桌面 IPC 注册表：前端可调用命令的唯一清单。
//!
//! 契约：任何 `#[tauri::command]` 必须在此登记后才对前端可见；
//! 新命令的接入步骤见项目 AGENTS.md「全局化接入契约」。

macro_rules! handler {
    () => {
        tauri::generate_handler![
            crate::commands::app::get_backend_info,
        ]
    };
}

pub(crate) use handler;
