#[path = "actions/keyboard.rs"]
pub(super) mod keyboard_actions;
use super::model::*;
use crate::{fanctl, hal::Hal, keyboard, keymap, lighting, osd, powerctl};
use anyhow::{Context, Result, bail};
use ccc_core::oem_events::Action;
use tauri::{AppHandle, Manager};

fn fan_identity(profile: &fanctl::FanProfile) -> Result<String> {
    digest(&(
        &profile.available_modes,
        profile.curve_kind,
        profile.fan_count,
        profile
            .fans
            .iter()
            .filter(|f| f.present)
            .map(|f| &f.id)
            .collect::<Vec<_>>(),
        profile.rpm_fans.iter().map(|f| &f.id).collect::<Vec<_>>(),
    ))
}
fn fn_lock_identity(app: &AppHandle, hal: &Hal) -> Result<String> {
    if app
        .state::<keyboard::Controller>()
        .read(hal)?
        .current
        .fn_locked
        .is_none()
    {
        bail!("固件未声明Fn锁");
    }
    Ok("fn-lock-supported".into())
}
fn optional_capability(scope: Scope, facts: &Facts) -> Result<String> {
    let (identity, error, fallback) = match scope {
        Scope::PanelHotkey => {
            let p = facts
                .panel_hotkey
                .as_ref()
                .context("屏幕按键能力读取失败")?;
            (&p.identity, &p.error, "屏幕按键不可用")
        }
        Scope::GpuTurbo => {
            let p = facts.gpu_turbo.as_ref().context("GPU Turbo 能力读取失败")?;
            (&p.identity, &p.error, "GPU Turbo 不可用")
        }
        Scope::BatteryThresholds => {
            let p = facts
                .battery_thresholds
                .as_ref()
                .context("电池阈值能力读取失败")?;
            (&p.identity, &p.error, "电池阈值不可用")
        }
        Scope::PowerDeliveryEvent => {
            let p = facts.pd_event.as_ref().context("PD事件能力读取失败")?;
            (&p.identity, &p.error, "PD事件不可用")
        }
        Scope::ScreenBrightness => {
            let p = facts
                .screen_brightness
                .as_ref()
                .context("屏幕亮度能力读取失败")?;
            (&p.identity, &p.error, "屏幕亮度能力不可用")
        }
        Scope::BatteryLowBrightness => {
            let p = facts
                .battery_low_brightness
                .as_ref()
                .context("低电量亮度能力读取失败")?;
            (&p.identity, &p.error, "低电量亮度不可用")
        }
        Scope::BatteryAlert => {
            let p = facts
                .battery_alert
                .as_ref()
                .context("电池音频事件能力读取失败")?;
            (&p.identity, &p.error, "电池音频事件不可用")
        }
        Scope::LidAction => {
            let p = facts.lid_action.as_ref().context("合盖计划读取失败")?;
            (&p.identity, &p.error, "合盖动作不可用")
        }
        Scope::BatteryLowAction => {
            let p = facts
                .battery_low_action
                .as_ref()
                .context("低电量系统动作读取失败")?;
            (&p.identity, &p.error, "低电量系统动作不可用")
        }
        _ => bail!("不是独立能力范围"),
    };
    identity
        .clone()
        .with_context(|| error.clone().unwrap_or_else(|| fallback.into()))
}
fn capability(app: &AppHandle, scope: Scope, facts: &Facts) -> Result<String> {
    let hal = app.state::<Hal>();
    match scope {
        Scope::Touchpad => app
            .state::<crate::touchpad::Controller>()
            .hotkey_identity(&hal),
        Scope::KeyboardColor => app
            .state::<lighting::LightingController>()
            .color_cycle_identity(&hal),
        Scope::KeyboardBrightnessEvent => app
            .state::<lighting::LightingController>()
            .brightness_event_identity(&hal),
        Scope::MicrophoneMute => app
            .state::<crate::microphone::Controller>()
            .hotkey_identity(&hal),
        Scope::PowerMode => facts
            .mode_cycle
            .as_ref()
            .context("模式循环能力读取失败")?
            .identity(),
        Scope::Fan => {
            let _gate = hal.begin_write()?;
            fan_identity(&fanctl::read_profile(&hal)?)
        }
        Scope::KeyboardBrightness | Scope::KeyboardSwitch => keyboard_identity(app),
        Scope::KeyboardZones => lighting::zone_hotkey::identity(
            &app.state::<lighting::LightingController>()
                .read_profile(&hal),
        ),
        Scope::FnLock => fn_lock_identity(app, &hal),
        Scope::Mappings => app.state::<keymap::Controller>().hotkey_identity(&hal),
        Scope::OpenConsole => Ok("main-window".into()),
        Scope::SystemRecovery => super::system_recovery::identity(),
        Scope::PanelEvent => super::panel_event::identity(&hal),
        Scope::AirplaneMode => {
            let profile = app.state::<crate::airplane::Controller>().read(&hal)?;
            profile
                .identity
                .with_context(|| profile.error.unwrap_or_else(|| "飞行模式不可用".into()))
        }
        Scope::SystemShutdown => super::system_action::shutdown_identity(&hal),
        Scope::StateNotifications => Ok(super::notifications::IDENTITY.into()),
        Scope::CameraReports => super::camera_report::identity(&hal),
        Scope::BatteryColdWarning => super::battery_cold::identity(&hal),
        Scope::OpenKeyboard => lighting::hotkey::navigation_identity(
            &app.state::<lighting::LightingController>()
                .read_profile(&hal),
        ),
        Scope::DisplayRotation => {
            let mode = app.state::<crate::display::Controller>().primary()?;
            mode.oem_rotation()?;
            digest(&(&mode.device, &mode.identity))
        }
        Scope::PanelHotkey
        | Scope::GpuTurbo
        | Scope::BatteryThresholds
        | Scope::PowerDeliveryEvent
        | Scope::ScreenBrightness
        | Scope::BatteryLowBrightness
        | Scope::BatteryLowAction
        | Scope::LidAction
        | Scope::BatteryAlert => optional_capability(scope, facts),
    }
}
fn queried<T>(enabled: bool, f: impl FnOnce() -> Result<T>) -> (Option<T>, Option<String>) {
    match enabled.then(f) {
        Some(Ok(value)) => (Some(value), None),
        Some(Err(error)) => (None, Some(format!("{error:#}"))),
        None => (None, None),
    }
}
fn mode_cycle(app: &AppHandle) -> Result<powerctl::mode::cycle::Profile> {
    let hal = app.state::<Hal>();
    let gpu = app.state::<crate::gpuctl::GpuController>();
    let power = app.state::<crate::gpu_power::Controller>();
    let _gate = hal.begin_write()?;
    powerctl::mode::cycle::read(&powerctl::mode::links::Context::native(&hal, &gpu, &power))
}
pub(super) fn facts(app: &AppHandle, scopes: &[Scope]) -> Facts {
    let (mode_cycle, mode_error) = queried(scopes.contains(&Scope::PowerMode), || mode_cycle(app));
    let (pd, pd_error) = queried(scopes.contains(&Scope::PowerDeliveryEvent), || {
        app.state::<crate::platform::Controller>()
            .read_pd_event(&app.state::<Hal>())
    });
    let (battery, battery_error) = queried(scopes.contains(&Scope::BatteryThresholds), || {
        app.state::<super::Controller>()
            .battery_thresholds
            .read(&app.state::<Hal>())
    });
    let (gpu, gpu_error) = queried(scopes.contains(&Scope::GpuTurbo), || {
        app.state::<crate::gpuctl::GpuController>()
            .read_turbo(&app.state::<Hal>())
    });
    let panel = scopes
        .contains(&Scope::PanelHotkey)
        .then(|| {
            app.state::<super::Controller>()
                .panel_hotkey
                .read(&app.state::<Hal>())
        })
        .and_then(Result::ok);
    let (screen, screen_error) = queried(scopes.contains(&Scope::ScreenBrightness), || {
        app.state::<super::Controller>()
            .screen_brightness
            .read(&app.state::<Hal>())
    });
    let (low, low_error) = queried(scopes.contains(&Scope::BatteryLowBrightness), || {
        app.state::<super::Controller>()
            .screen_brightness
            .read_low(&app.state::<Hal>())
    });
    let (alert, alert_error) = queried(scopes.contains(&Scope::BatteryAlert), || {
        let controller = app.state::<super::Controller>();
        controller
            .battery_alert
            .read(&app.state::<Hal>(), &controller.screen_brightness)
    });
    let system = scopes
        .contains(&Scope::BatteryLowAction)
        .then(|| super::low_battery_action::profile(&app.state::<Hal>()));
    let errors = [
        (Scope::PowerMode, mode_error),
        (Scope::ScreenBrightness, screen_error),
        (Scope::GpuTurbo, gpu_error),
        (Scope::BatteryThresholds, battery_error),
        (Scope::PowerDeliveryEvent, pd_error),
        (Scope::BatteryLowBrightness, low_error),
        (Scope::BatteryAlert, alert_error),
    ];
    let mut facts = Facts {
        mode_cycle,
        panel_hotkey: panel,
        screen_brightness: screen,
        gpu_turbo: gpu,
        battery_thresholds: battery,
        pd_event: pd,
        battery_low_brightness: low,
        battery_low_action: system,
        lid_action: scopes
            .contains(&Scope::LidAction)
            .then(|| super::system_action::profile(&app.state::<Hal>())),
        battery_alert: alert,
        capabilities: vec![],
        blocked_reason: crate::hal::oem_owner::ensure_owner_absent()
            .err()
            .map(|e| format!("{e:#}")),
    };
    fill_capabilities(app, scopes, &mut facts, &errors);
    facts
}
fn fill_capabilities(
    app: &AppHandle,
    scopes: &[Scope],
    facts: &mut Facts,
    errors: &[(Scope, Option<String>)],
) {
    facts.capabilities = scopes
        .iter()
        .map(|scope| {
            let error = errors
                .iter()
                .find(|(key, _)| key == scope)
                .and_then(|(_, error)| error.as_deref());
            let result = match error {
                Some(error) => Err(anyhow::anyhow!("{error}")),
                None => capability(app, *scope, facts),
            };
            Capability {
                scope: *scope,
                reason: result.as_ref().err().map(|e| format!("{e:#}")),
                identity: result.ok(),
            }
        })
        .collect();
}
fn keyboard_identity(app: &AppHandle) -> Result<String> {
    let hal = app.state::<Hal>();
    let profile = app
        .state::<lighting::LightingController>()
        .read_profile(&hal);
    match profile.detected_family {
        lighting::KeyboardFamily::Perkey => app
            .state::<lighting::perkey::Controller>()
            .hotkey_identity(&hal)
            .map(|s| format!("perkey:{s}")),
        lighting::KeyboardFamily::MultiZone => app
            .state::<lighting::multizone::Controller>()
            .hotkey_identity(&hal)
            .map(|s| format!("multiZone:{s}")),
        _ => lighting::hotkey::identity(&profile),
    }
}
fn require(actual: String, expected: &str) -> Result<()> {
    if actual != expected {
        bail!("热键目标能力变化，请重新确认接管范围");
    }
    Ok(())
}
pub(super) struct Completion {
    pub message: Option<osd::Message>,
    pub show_osd: bool,
}
pub(super) fn execute(app: &AppHandle, action: Action, expected: &str) -> Result<Completion> {
    let hal = app.state::<Hal>();
    let message = match action {
        Action::Notify(notice) => {
            return super::notifications::message(notice, expected, || hal.usb_keyboard_present())
                .map(|message| Completion {
                    message,
                    show_osd: true,
                });
        }
        Action::CycleMode => bail!("模式循环必须通过持有完整GPU授权的事件会话执行"),
        Action::SystemShutdown
        | Action::LidAction
        | Action::OpenRecovery
        | Action::ToggleAirplane
        | Action::PanelEvent => {
            bail!("系统事件缺少可取消执行上下文")
        }
        Action::DisplayBrightness(_) => bail!("屏幕亮度策略缺少可取消事件上下文"),
        Action::PanelHotkey => bail!("屏幕按键缺少可取消事件上下文"),
        Action::GpuTurbo(_) => bail!("GPU Turbo 缺少可取消事件上下文"),
        Action::BatteryThreshold(_) => bail!("电池阈值缺少可取消事件上下文"),
        Action::PowerDeliveryEvent => bail!("PD事件缺少可取消事件上下文"),
        Action::BatteryColdWarning => bail!("低温提醒缺少可取消事件上下文"),
        Action::BatteryLowBrightness(_) => bail!("低电量亮度缺少可取消事件上下文"),
        Action::BatteryAlert { .. } => bail!("电池音频事件缺少可取消会话"),
        Action::BatteryLowAction(_) => bail!("低电量系统动作缺少可取消事件上下文"),
        Action::OpenKeyboard => bail!("键盘导航缺少可取消事件上下文"),
        Action::DisplayRotation => bail!("显示导航缺少可取消事件上下文"),
        Action::CameraReport(report) => super::camera_report::handle(&hal, expected, report),
        Action::FanMode(mode) => fan_mode(app, expected, mode),
        Action::KeyboardBrightness(delta) => keyboard_actions::brightness(app, expected, delta),
        Action::KeyboardBrightnessEvent => {
            return keyboard_actions::brightness_event(app, expected);
        }
        Action::ToggleKeyboard { rgb_only } => {
            keyboard_actions::keyboard_switch(app, expected, rgb_only)
        }
        Action::ToggleMicrophones => microphone_mute(app, expected),
        Action::ToggleTouchpad => touchpad_switch(app, expected),
        Action::ToggleKeyboardZone(zone) => keyboard_actions::keyboard_zone(app, expected, zone),
        Action::CycleKeyboardColor => keyboard_actions::keyboard_color(app, expected),
        Action::ToggleFnLock => keyboard_actions::fn_lock(app, expected),
        Action::ToggleMappings => {
            let enabled = app
                .state::<keymap::Controller>()
                .toggle_owned(&hal, expected)?;
            Ok(osd::Message::result(
                "按键映射",
                if enabled {
                    "已开启已接管设备的映射"
                } else {
                    "已关闭已接管设备的映射"
                },
            ))
        }
        Action::OpenControlCenter => {
            require("main-window".into(), expected)?;
            let handle = app.clone();
            app.run_on_main_thread(move || crate::tray::show_main(&handle))?;
            Ok(osd::Message::result("主窗口", "已请求显示窗口"))
        }
    }?;
    Ok(Completion {
        message: Some(message),
        show_osd: true,
    })
}
fn microphone_mute(app: &AppHandle, expected: &str) -> Result<osd::Message> {
    let muted = app
        .state::<crate::microphone::Controller>()
        .toggle(&app.state::<Hal>(), expected)?;
    Ok(osd::Message::result(
        "麦克风",
        if muted {
            "全部已静音"
        } else {
            "全部已取消静音"
        },
    ))
}

fn touchpad_switch(app: &AppHandle, expected: &str) -> Result<osd::Message> {
    let enabled = app
        .state::<crate::touchpad::Controller>()
        .toggle(&app.state::<Hal>(), expected)?;
    Ok(osd::Message::result(
        "触摸板",
        if enabled { "已启用" } else { "已关闭" },
    ))
}

fn fan_mode(app: &AppHandle, expected: &str, mode: u8) -> Result<osd::Message> {
    let hal = app.state::<Hal>();
    let _gate = hal.begin_write()?;
    let profile = fanctl::read_profile(&hal)?;
    require(fan_identity(&profile)?, expected)?;
    fanctl::set_mode(&hal, mode)?;
    Ok(osd::Message::result(
        "风扇模式",
        match mode {
            0 => "自动",
            1 => "全速",
            5 => "MaxQ",
            6 => "已保存的自定义设置",
            _ => "未知",
        },
    ))
}
