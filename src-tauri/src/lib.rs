//! 应用组装入口：模块声明 + Builder 装配（插件、状态、命令注册表）。
//! 平台/窗口生命周期与命令注册分离，命令清单见 command_registry.rs。

mod command_registry;
mod commands;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(command_registry::handler!())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
