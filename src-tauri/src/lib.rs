//! 应用组装入口：模块声明 + Builder 装配（插件、状态、命令注册表）。
//! 平台/窗口生命周期与命令注册分离，命令清单见 command_registry.rs。

mod airplane;
mod assembly;
mod battery;
mod command_registry;
mod commands;
mod controller_lease;
mod display;
mod fanctl;
mod gpu_power;
mod gpuctl;
mod graphics_mode;
mod hal;
mod hotkeys;
mod keyboard;
mod keymap;
mod lighting;
mod microphone;
mod osd;
#[cfg(not(test))]
mod owned_process;
mod platform;
mod powerctl;
mod preferences;
mod profiles;
mod startup;
mod storage;
mod touchpad;
mod tray;
mod watchdog;
mod worker_entry;

use hal::Hal;
use tauri::{Emitter, Manager};
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(code) = worker_entry::run(&args) {
        std::process::exit(code as i32);
    }
    let mut context = tauri::generate_context!();
    let restart_lease =
        preferences::pending_lease().expect("等待原控制会话释放失败，未重新接管硬件");
    let windows = preferences::prepare_windows(&mut context);
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _| {
            startup::on_second_launch(app, &args);
        }))
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Trace)
                .max_file_size(2_000_000)
                .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepSome(2))
                .targets([tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::LogDir {
                        file_name: Some("colorful-control-center".into()),
                    },
                )])
                .build(),
        )
        .plugin(preferences::plugin())
        .plugin(preferences::shortcut_plugin())
        .plugin(control_plugin(restart_lease))
        .plugin(tauri_plugin_opener::init())
        .manage(startup::LaunchReport::default())
        .invoke_handler(command_registry::handler!())
        .setup(move |app| {
            preferences::create_window(app.handle(), &windows)?;
            tray::setup(app.handle())?;
            startup::on_launch(app.handle());
            start_workers(app.handle());
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "main"
                && matches!(
                    event,
                    tauri::WindowEvent::Focused(false) | tauri::WindowEvent::Destroyed
                )
            {
                if let Some(controller) = window.app_handle().try_state::<keymap::Controller>() {
                    if let Err(error) = controller.stop_recording() {
                        log::error!("[keymap] 停止录制失败：{error:#}");
                    }
                }
            }
        })
        .build(context)
        .expect("error while building tauri application")
        .run(on_run_event);
}

fn on_run_event(app: &tauri::AppHandle, event: tauri::RunEvent) {
    if matches!(event, tauri::RunEvent::Exit) {
        #[cfg(not(test))]
        if let Err(error) = app
            .state::<powerctl::mode::supply::Controller>()
            .stop_worker()
        {
            log::error!("[supply-mode] 退出停止：{error:#}");
        }
        #[cfg(not(test))]
        if let Err(error) = app.state::<display::automatic::Controller>().stop_worker() {
            log::error!("[auto-refresh] 退出停止：{error:#}");
        }
        if let Err(error) = app.state::<profiles::policy::Controller>().stop() {
            log::error!("[power-policy] 退出停止：{error:#}");
        }
        if let Err(error) = app.state::<hotkeys::Controller>().stop(app) {
            log::error!("[hotkeys] 退出停止：{error:#}");
        }
        app.state::<osd::Controller>().stop();
        if let Err(error) = fanctl::release_on_exit(&app.state::<Hal>()) {
            log::error!("[software-fan] 退出恢复：{error:#}");
        }
    }
}

fn start_workers(app: &tauri::AppHandle) {
    #[cfg(not(test))]
    if let Err(error) = app
        .state::<display::automatic::Controller>()
        .start_worker(app)
    {
        log::error!("[auto-refresh] 启动：{error:#}");
    }
    #[cfg(not(test))]
    if let Err(error) = app
        .state::<powerctl::mode::supply::Controller>()
        .start_worker(app)
    {
        log::error!("[supply-mode] 启动：{error:#}");
    }
    #[cfg(not(test))]
    if let Err(error) = app.state::<profiles::policy::Controller>().start(app) {
        log::error!("[power-policy] 启动：{error:#}");
    }
    let hotkey_handle = app.clone();
    std::thread::spawn(move || {
        hotkey_handle
            .state::<hotkeys::Controller>()
            .resume_saved(&hotkey_handle)
    });
    start_hardware_workers(app);
}
fn start_hardware_workers(app: &tauri::AppHandle) {
    let fan_handle = app.clone();
    std::thread::spawn(move || {
        loop {
            if let Some(hal) = fan_handle.try_state::<Hal>() {
                if let Err(error) = fanctl::software::tick(&hal) {
                    log::error!("[software-fan] {error:#}");
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
    });
    let guard_handle = app.clone();
    std::thread::spawn(move || {
        loop {
            if let Some(hal) = guard_handle.try_state::<Hal>() {
                if let Err(error) = hal.supervise_fan_guard() {
                    log::error!("[watchdog] {error:#}");
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
    });
    let display_handle = app.clone();
    let battery_handle = app.clone();
    std::thread::spawn(move || {
        loop {
            if let Err(error) = battery_handle
                .state::<battery::Controller>()
                .supervise_calibration(&battery_handle.state::<Hal>())
            {
                log::error!("[battery-calibration] {error:#}");
            }
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
    });
    std::thread::spawn(move || {
        loop {
            if let Err(error) = display_handle
                .state::<display::Controller>()
                .tick_runtime(&display_handle.state::<Hal>())
            {
                log::error!("[display-restore] {error:#}");
            }
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
    });
    // 采样与监护心跳独立，硬件读取阻塞不阻断监护管道。
    let handle = app.clone();
    std::thread::spawn(move || {
        loop {
            if let Some(hal) = handle.try_state::<Hal>() {
                let mut snapshot = hal.display_snapshot();
                let safety = hal.safety_status();
                if safety != "自动" && safety != "监护中" {
                    snapshot.warnings.push(safety);
                }
                let _ = handle.emit("hardware-snapshot", snapshot);
            }
            std::thread::sleep(std::time::Duration::from_millis(1000));
        }
    });
}

fn control_plugin(
    restart_lease: Option<controller_lease::ControllerLease>,
) -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri::plugin::Builder::new("controller")
        .setup(move |app, _| {
            let lease = match restart_lease {
                Some(lease) => lease,
                None => controller_lease::ControllerLease::try_acquire(controller_lease::NAME)?
                    .ok_or("已有控制台或独立恢复任务仍在运行，请等待恢复结束后重试")?,
            };
            app.manage(lease);
            assembly::setup(app)?;
            Ok(())
        })
        .build()
}
