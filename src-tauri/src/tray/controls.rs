use super::native::{self, Control, Controls, FanAction, Fans, Presets, Tray};
use crate::{
    assembly::mode_context,
    display,
    fanctl::{self, FAN_MODE_AUTO, FAN_MODE_CUSTOM, FAN_MODE_FULL_SPEED, FAN_MODE_NOISELESS},
    hal::{Hal, HardwareSnapshot},
    osd,
    powerctl::mode::{self, MODE_LABELS, Target, manual, selection},
    shutdown,
};
use anyhow::{Context, Result, anyhow, ensure};
use ccc_core::software_fan::Curves;
use tauri::{AppHandle, Emitter, Manager};

pub(crate) fn sample_tooltip(app: &AppHandle, hal: &Hal, hardware: Option<&HardwareSnapshot>) {
    let Some(tray) = app.try_state::<Tray>() else {
        return;
    };
    let Some(_operation) = shutdown::enter() else {
        return;
    };
    let (cpu, gpu) = hardware.map_or_else(
        || hal.tray_temperatures(),
        |hardware| {
            let gpu = (!hal.gpu_suspended())
                .then_some(hardware.nvidia.as_ref())
                .flatten()
                .and_then(|gpu| gpu.temp_c);
            (hardware.fan.cpu.temp_c, gpu)
        },
    );
    let temperatures = temperatures(cpu, gpu);
    let gate = hal.control_gate();
    let Ok(_gate) = gate.try_lock() else {
        tray.set_temperatures(temperatures);
        return;
    };
    let preset = hal
        .with_dchu(mode::current_mode)
        .ok()
        .and_then(|mode| MODE_LABELS.get(usize::from(mode)).copied())
        .unwrap_or("不可用");
    let fan = tooltip_fan(hal).unwrap_or("不可用");
    tray.set_telemetry(preset, fan, temperatures);
}

fn tooltip_fan(hal: &Hal) -> Result<&'static str> {
    let mode = hal.with_dchu(|io| io.read_fan_mode())?;
    if hal.fan_recovery_pending() {
        Ok("正在恢复")
    } else if fanctl::software::read(hal)?.is_some() {
        Ok("软件曲线接管")
    } else {
        Ok(fan_label(mode))
    }
}

pub(super) fn menu_controls(app: &AppHandle) -> Controls {
    let read = || -> Result<Controls> {
        let _operation = shutdown::enter().context(shutdown::CLOSING)?;
        let hal = app.try_state::<Hal>().context("硬件状态尚未初始化")?;
        app.try_state::<crate::display::Controller>()
            .context("显示控制尚未初始化")?;
        let gate = hal.control_gate();
        let _gate = gate
            .try_lock()
            .map_err(|error| anyhow!("控制事务暂不可读：{error}"))?;
        Ok(read_controls(app, &hal))
    };
    read().unwrap_or_else(|error| Controls {
        presets: Err(format!("{error:#}")),
        fans: Err(format!("{error:#}")),
    })
}

fn read_controls(app: &AppHandle, hal: &Hal) -> Controls {
    let blocked = fan_access(hal).err().map(|error| format!("{error:#}"));
    let presets = read_presets(app, hal, blocked.clone());
    let fans = read_fans(hal, blocked);
    Controls {
        presets: presets.map_err(|error| format!("预设状态不可用：{error:#}")),
        fans: fans.map_err(|error| format!("风扇状态不可用：{error:#}")),
    }
}

fn read_presets(app: &AppHandle, hal: &Hal, blocked: Option<String>) -> Result<Presets> {
    let (current, available, support_issue) = hal.with_dchu(|io| {
        let (available, issue) = mode::mode_choices(io)?;
        Ok((mode::current_mode(io)?, available, issue))
    })?;
    let choices = [1, 0, 3, 2, 4]
        .into_iter()
        .filter(|mode| available.contains(mode))
        .map(|mode| (mode, MODE_LABELS[usize::from(mode)]))
        .collect::<Vec<_>>();
    let label = MODE_LABELS.get(usize::from(current));
    let blocked = support_issue
        .or_else(|| choices.is_empty().then(|| "固件未提供可切换的预设".into()))
        .or_else(|| label.is_none().then(|| "当前预设未识别".into()))
        .or(blocked)
        .or_else(|| {
            manual::ensure_available(hal, &app.state::<display::Controller>())
                .err()
                .map(|error| format!("{error:#}"))
        });
    Ok(Presets {
        current,
        label: label.copied().unwrap_or("未识别"),
        choices,
        blocked,
    })
}

fn read_fans(hal: &Hal, blocked: Option<String>) -> Result<Fans> {
    let profile = fanctl::read_firmware_profile(hal)?;
    let software = fanctl::software::read(hal)?.is_some();
    let recovering = hal.fan_recovery_pending();
    let active = match (software || recovering, profile.current_mode) {
        (false, FAN_MODE_AUTO) => Some(FanAction::Auto),
        (false, FAN_MODE_NOISELESS)
            if fanctl::automatic_mode(hal).ok() == Some(FAN_MODE_NOISELESS) =>
        {
            Some(FanAction::Auto)
        }
        (false, FAN_MODE_FULL_SPEED) => Some(FanAction::FullSpeed),
        _ => None,
    };
    let choices = [FanAction::Auto, FanAction::FullSpeed]
        .into_iter()
        .filter(|action| profile.available_modes.contains(&fan_target(*action)))
        .collect();
    let label = if recovering {
        "正在恢复"
    } else if software {
        "软件曲线接管"
    } else {
        fan_label(profile.current_mode)
    };
    Ok(Fans {
        current: profile.current_mode,
        label,
        active,
        choices,
        blocked,
    })
}

fn temperatures(cpu: Option<f64>, gpu: Option<f64>) -> String {
    let readings = [("CPU", cpu), ("GPU", gpu)]
        .into_iter()
        .filter_map(|(name, value)| {
            value
                .filter(|temp| temp.is_finite())
                .map(|temp| format!("{name} {temp:.0}°C"))
        })
        .collect::<Vec<_>>();
    if readings.is_empty() {
        String::new()
    } else {
        format!("\n{}", readings.join(" · "))
    }
}

pub(super) fn dispatch(app: &AppHandle, control: Control) {
    let Some(tray) = app.try_state::<Tray>() else {
        return;
    };
    if shutdown::closing() || !tray.begin_control() {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let tray = app.state::<Tray>();
        let outcome = match control {
            Control::Preset(mode) => switch_preset(&app, mode),
            Control::Fan(action) => switch_fan(&app.state::<Hal>(), action, native::confirm),
        };
        if !matches!(outcome, Ok(None)) && !shutdown::closing() {
            let result = read_back(&app, &tray, control, outcome);
            if !shutdown::closing() {
                report(&app, control, result);
            }
        }
        tray.end_control();
    });
}

fn admitted<T>(work: impl FnOnce() -> Result<T>) -> Result<T> {
    let _operation = shutdown::enter().context(shutdown::CLOSING)?;
    work()
}

fn preset_target(app: &AppHandle, wanted: u8) -> Result<(Target, bool)> {
    let context = mode_context(app);
    let hal = context.hal;
    let _gate = hal.begin_write()?;
    manual::ensure_available(hal, &app.state::<display::Controller>())?;
    fan_access(hal)?;
    let plan = selection::checkpoint(&context, wanted, None)?;
    let software = fanctl::software::read(hal)?.is_some();
    Ok((plan.target, software))
}

fn switch_preset(app: &AppHandle, wanted: u8) -> Result<Option<u8>> {
    let label = MODE_LABELS
        .get(usize::from(wanted))
        .context("未知的预设模式")?;
    let (target, software) = admitted(|| preset_target(app, wanted))?;
    if software
        && !native::confirm(
            "切换预设模式",
            &format!("切换到「{label}」会停止当前的软件风扇曲线。是否继续？"),
        )?
    {
        return Ok(None);
    }
    let apply = |approval| {
        admitted(|| {
            manual::apply(
                mode_context(app),
                &app.state::<display::Controller>(),
                &target,
                approval,
            )
        })
    };
    let manual::Apply::ConfirmFnKey(approval) = apply(None)? else {
        return Ok(Some(wanted));
    };
    if !native::confirm(
        "关闭原厂 FnKey",
        &format!(
            "关闭原厂 FnKey 后将切换到「{label}」。部分原厂 Fn 热键和屏显会暂停，可从开始菜单恢复。是否继续？"
        ),
    )? {
        return Ok(None);
    }
    ensure!(
        matches!(apply(Some(approval))?, manual::Apply::Applied),
        "原厂 FnKey 状态已变化，请重新选择模式"
    );
    Ok(Some(wanted))
}

#[derive(PartialEq)]
struct FanPreview {
    preset: u8,
    current: u8,
    software: Option<Curves>,
}

fn fan_access(hal: &Hal) -> Result<()> {
    fanctl::ensure_manual_allowed(hal)
}

fn fan_preview(hal: &Hal, target: u8) -> Result<FanPreview> {
    fan_access(hal)?;
    let firmware = fanctl::read_firmware_profile(hal)?;
    ensure!(
        firmware.available_modes.contains(&target),
        "固件未声明支持此风扇模式"
    );
    Ok(FanPreview {
        preset: hal.with_dchu(mode::current_mode)?,
        current: firmware.current_mode,
        software: fanctl::software::read(hal)?,
    })
}

fn switch_fan(
    hal: &Hal,
    action: FanAction,
    confirm: impl FnOnce(&str, &str) -> Result<bool>,
) -> Result<Option<u8>> {
    let target = fan_target(action);
    if action == FanAction::Auto {
        return admitted(|| {
            let _gate = hal.begin_write()?;
            fan_access(hal)?;
            fanctl::set_mode(hal, target).map(Some)
        });
    }
    let before = admitted(|| {
        let _gate = hal.begin_write()?;
        fan_preview(hal, target)
    })?;
    if before.current == target && before.software.is_none() {
        return Ok(None);
    }
    let detail = if before.software.is_some() {
        "切换到「全速」会停止当前的软件风扇曲线。之后可在托盘选择「自动」。是否继续？"
    } else {
        "将风扇切换到「全速」。之后可在托盘选择「自动」。是否继续？"
    };
    if !confirm("确认风扇全速", detail)? {
        return Ok(None);
    }
    admitted(|| {
        let _gate = hal.begin_write()?;
        ensure!(
            fan_preview(hal, target)? == before,
            "风扇或预设状态已变化，请重新选择全速"
        );
        fanctl::set_mode(hal, target).map(Some)
    })
}

fn fan_target(action: FanAction) -> u8 {
    match action {
        FanAction::Auto => FAN_MODE_AUTO,
        FanAction::FullSpeed => FAN_MODE_FULL_SPEED,
    }
}

fn fan_label(mode: u8) -> &'static str {
    match mode {
        FAN_MODE_AUTO => "自动",
        FAN_MODE_FULL_SPEED => "全速",
        5 => "能效优先",
        FAN_MODE_CUSTOM => "自定义",
        FAN_MODE_NOISELESS => fanctl::NOISELESS_FAN_LABEL,
        _ => "模式未识别",
    }
}

fn read_back(
    app: &AppHandle,
    tray: &Tray,
    control: Control,
    outcome: Result<Option<u8>>,
) -> Result<&'static str> {
    admitted(|| {
        let hal = app.state::<Hal>();
        let _gate = hal.begin_write()?;
        let controls = read_controls(app, &hal);
        let outcome =
            outcome.and_then(|mode| verify(control, mode.context("缺少应用目标")?, &controls));
        tray.set_control_labels(&controls);
        outcome
    })
}

fn verify(control: Control, expected: u8, controls: &Controls) -> Result<&'static str> {
    match control {
        Control::Preset(_) => {
            let presets = controls
                .presets
                .as_ref()
                .map_err(|error| anyhow!("{error}"))?;
            ensure!(
                presets.current == expected,
                "设备回读的预设为{}，与所选模式不同",
                presets.label
            );
            Ok(presets.label)
        }
        Control::Fan(action) => {
            let fans = controls.fans.as_ref().map_err(|error| anyhow!("{error}"))?;
            ensure!(
                fans.current == expected && fans.active == Some(action),
                "设备回读的风扇状态为{}，与所选模式不同",
                fans.label
            );
            Ok(fans.label)
        }
    }
}

fn report(app: &AppHandle, control: Control, result: Result<&'static str>) {
    if let Err(error) = app.emit_to("main", "hardware-controls-changed", ()) {
        log::error!("[tray] 刷新主窗口：{error:#}");
    }
    let title = match control {
        Control::Preset(_) => "预设模式",
        Control::Fan(_) => "风扇散热",
    };
    let mut message = match &result {
        Ok(label) => osd::Message::result(title, format!("已切换至{label}")),
        Err(error) => {
            log::error!("[tray] {title}：{error:#}");
            osd::Message::result(
                title,
                format!("{error:#}").chars().take(512).collect::<String>(),
            )
        }
    };
    message.error = result.is_err();
    if let Err(error) = app.state::<osd::Controller>().show(app, message) {
        log::error!("[tray] 屏显：{error:#}");
    }
}

#[cfg(test)]
mod tests;
