//! 应用组装入口：模块声明 + Builder 装配（插件、状态、命令注册表）。
//! 平台/窗口生命周期与命令注册分离，命令清单见 command_registry.rs。

mod airplane;
mod app_update;
mod assembly;
mod battery;
mod bundled_fonts;
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
mod monitoring;
mod osd;
#[cfg(not(test))]
mod owned_process;
mod platform;
mod powerctl;
mod preferences;
mod profiles;
mod shutdown;
mod startup;
mod storage;
mod touchpad;
mod tray;
mod watchdog;
mod worker_entry;

use hal::Hal;
use tauri::Manager;
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(code) = worker_entry::run(&args) {
        std::process::exit(code as i32);
    }
    let result =
        startup::prepare_desktop(&args).and_then(|start| start.map(run_desktop).transpose());
    if let Err(error) = result {
        startup::report_start_failure(&error);
        std::process::exit(1);
    }
}

fn run_desktop(start: startup::DesktopStart) -> anyhow::Result<()> {
    let startup::DesktopStart {
        activation,
        lease,
        login,
        reveal,
    } = start;
    let mut context = tauri::generate_context!();
    let windows = preferences::prepare_windows(&mut context, &activation.window_class);
    tauri::Builder::default()
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
        .plugin(shutdown::plugin())
        .plugin(preferences::plugin())
        .plugin(preferences::shortcut_plugin())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .register_asynchronous_uri_scheme_protocol("cccfonts", bundled_fonts::serve)
        .manage(lease)
        .manage(startup::LaunchReport::default())
        .manage(startup::Preparation::default())
        .invoke_handler(command_registry::handler!())
        .setup(move |app| {
            let Some(_operation) = shutdown::enter() else {
                return Ok(());
            };
            let preparation = app.state::<startup::Preparation>();
            preparation.set_appearance(app.state::<preferences::Controller>().snapshot().ok());
            preferences::create_window(app.handle(), &windows)?;
            tray::bind_window(app.handle());
            activation.listen(app.handle())?;
            if reveal {
                tray::show_main(app.handle());
            }
            preparation.start(app.handle(), login);
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "main"
                && !shutdown::closing()
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
        .build(context)?
        .run(shutdown::on_event);
    Ok(())
}

fn start_workers(app: &tauri::AppHandle) {
    if shutdown::closing() {
        return;
    }
    startup::start_oem_worker(app);
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
        if let Some(_operation) = shutdown::enter() {
            hotkey_handle
                .state::<hotkeys::Controller>()
                .resume_saved(&hotkey_handle);
        }
    });
    start_hardware_workers(app);
}
fn start_software_fan_workers(app: &tauri::AppHandle) {
    if !app
        .try_state::<Hal>()
        .is_some_and(|hal| hal.software_fan_supported())
    {
        return;
    }
    let fan_handle = app.clone();
    std::thread::spawn(move || {
        while let Some(operation) = shutdown::enter() {
            if let Some(hal) = fan_handle.try_state::<Hal>() {
                if let Err(error) = fanctl::software::tick(&hal) {
                    log::error!("[software-fan] {error:#}");
                }
            }
            drop(operation);
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
}

fn start_hardware_workers(app: &tauri::AppHandle) {
    start_software_fan_workers(app);
    let display_handle = app.clone();
    let battery_handle = app.clone();
    std::thread::spawn(move || {
        while let Some(operation) = shutdown::enter() {
            if let Err(error) = battery_handle
                .state::<battery::Controller>()
                .supervise_calibration(&battery_handle.state::<Hal>())
            {
                log::error!("[battery-calibration] {error:#}");
            }
            drop(operation);
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
    });
    std::thread::spawn(move || {
        while let Some(operation) = shutdown::enter() {
            if let Err(error) = display_handle
                .state::<display::Controller>()
                .tick_runtime(&display_handle.state::<Hal>())
            {
                log::error!("[display-restore] {error:#}");
            }
            drop(operation);
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
    });
    // 采样与监护心跳独立，硬件读取阻塞不阻断监护管道。
    monitoring::start(app);
}
