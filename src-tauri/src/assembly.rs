//! 应用领域状态的唯一装配；实际设备连接仅发生在正常原生启动。
use crate::{
    battery, display, fanctl, gpu_power, gpuctl, graphics_mode, hal::Hal, hotkeys, keyboard,
    keymap, lighting, microphone, osd, platform, powerctl, profiles, touchpad,
};
use anyhow::Result;
use tauri::Manager;

pub(crate) fn mode_context(app: &tauri::AppHandle) -> powerctl::mode::links::Context<'_> {
    powerctl::mode::links::Context::native(
        app.state::<Hal>().inner(),
        app.state::<gpuctl::GpuController>().inner(),
        app.state::<gpu_power::Controller>().inner(),
    )
}

pub(super) fn setup(app: &tauri::AppHandle) -> Result<()> {
    let dir = app.path().app_data_dir()?;
    let profile_policy = dir.join("power-profile-policy.json");
    let mut hal = Hal::new();
    hal.bind_software_fan_record(dir.join(crate::hal::SOFTWARE_FAN_RECORD_NAME));
    hal.bind_calibration_record(dir.join(battery::calibration::RECORD_NAME));
    hal.bind_automatic_mode_record(dir.join(powerctl::mode::supply::RECORD_NAME));
    let clocks = gpuctl::GpuController::new();
    let power = gpu_power::Controller::default();
    if let Err(error) = power.read_runtime(&hal, &clocks) {
        log::warn!("[gpu-power] 初始状态：{error:#}");
    }
    app.manage(hal);
    app.manage(clocks);
    app.manage(power);
    app.manage(display::Controller::new(display::native::Native));
    app.manage(display::automatic::Controller::new(
        dir.join("oem-auto-refresh.json"),
    ));
    app.manage(powerctl::mode::supply::Controller::new(
        dir.join(powerctl::mode::supply::RECORD_NAME),
        profile_policy.clone(),
    ));
    app.manage(profiles::application::Controller::default());
    app.manage(profiles::policy::Controller::new(profile_policy));
    app.manage(graphics_mode::Controller::new(
        dir.join("graphics-mode.json"),
    ));
    app.manage(battery::Controller::new(dir.join("energy-policy.json")));
    app.manage(platform::Controller::default());
    app.manage(platform::usb::Controller::default());
    app.manage(platform::lan::Controller::new(
        dir.join(platform::lan::RECORD_NAME),
    ));
    app.manage(fanctl::dust::Controller::default());
    input(app, &dir);
    Ok(())
}
fn input(app: &tauri::AppHandle, dir: &std::path::Path) {
    app.manage(crate::airplane::Controller::default());
    app.manage(lighting::LightingController::default());
    app.manage(microphone::Controller::default());
    app.manage(touchpad::Controller::default());
    app.manage(lighting::options::Controller::default());
    app.manage(lighting::lightbar::Controller::default());
    app.manage(lighting::multizone::Controller::default());
    app.manage(lighting::perkey::Controller::default());
    app.manage(keyboard::Controller::default());
    app.manage(osd::Controller::default());
    app.manage(hotkeys::Controller::new(dir.join("oem-hotkeys.json")));
    app.manage(keymap::Controller::new(dir.join("keymap-recovery.json")));
}
