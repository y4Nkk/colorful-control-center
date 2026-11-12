//! 应用组装入口：模块声明 + Builder 装配（插件、状态、命令注册表）。
//! 平台/窗口生命周期与命令注册分离，命令清单见 command_registry.rs。

mod command_registry;
mod commands;
mod fanctl;
mod hal;

use hal::Hal;
use tauri::{Emitter, Manager};
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(Hal::new())
        .invoke_handler(command_registry::handler!())
        .setup(|app| {
            // 1Hz 硬件快照轮询线程：常驻 emit，前端仅在监控页可见时订阅
            let handle = app.handle().clone();
            std::thread::spawn(move || loop {
                if let Some(hal) = handle.try_state::<Hal>() {
                    let _ = handle.emit("hardware-snapshot", hal.snapshot());
                }
                std::thread::sleep(std::time::Duration::from_millis(1000));
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
