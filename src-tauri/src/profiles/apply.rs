//! 档案全量预检 → 顺序写入 → 首错停止 → 恢复本次应用前快照。
use super::{ApplyStep, ProfileSettings};
use super::{Devices, devices::Scopes};
use crate::{
    fanctl,
    gpuctl::{GpuCheckpoint, GpuController},
    hal::Hal,
    lighting::lightbar,
    powerctl,
};
use anyhow::{Result, bail};
use ccc_core::settings::Patch;
#[path = "apply/preflight.rs"]
mod preflight;
use preflight::{points, preflight};
#[path = "apply/graphics.rs"]
mod graphics;
#[path = "apply/keyboard.rs"]
mod keyboard;
#[path = "apply/mux.rs"]
mod mux;
#[path = "apply/platform.rs"]
mod platform;
#[path = "apply/preparation.rs"]
mod preparation;
pub(super) mod review;
use preparation::transact;
mod execution;
use execution::transact_locked;

pub(super) struct Outcome {
    pub(super) steps: Vec<ApplyStep>,
    pub(super) targets_applied: bool,
}

struct Checkpoint {
    display: Option<crate::display::profile::Review>,
    mode: Option<powerctl::mode::links::Plan>,
    options: Option<powerctl::mode::boost::Plan>,
    fan: Option<fanctl::FanCheckpoint>,
    power: Vec<Patch>,
    graphics: graphics::Plan,
    mux: Option<crate::graphics_mode::profile::Review>,
    runtime: Option<powerctl::runtime::Plan>,
    light_bar: Option<lightbar::Plan>,
    dust: Option<fanctl::dust::Plan>,
    keyboard: keyboard::Plan,
    microphones: Option<crate::microphone::Plan>,
    touchpad: Option<crate::touchpad::Plan>,
    platform: platform::Plan,
}

#[derive(Default)]
struct Touched {
    mode: bool,
    options: powerctl::mode::boost::Change,
    fan: bool,
    saved: bool,
    runtime: bool,
    gpu: bool,
    light_bar: bool,
    dust: bool,
    keyboard: keyboard::Touched,
    microphones: bool,
    touchpad: bool,
    platform: platform::Touched,
}

#[derive(Clone, Copy)]
enum ModeReview<'a> {
    #[cfg(test)]
    Unattended,
    Confirmed {
        permission: Option<&'a super::application::Permission>,
        mode: Option<&'a powerctl::mode::Target>,
        source: Option<(&'a super::application::Source, &'a super::ProfileFile)>,
        display: Option<&'a crate::display::profile::Review>,
        gpu_power: Option<&'a crate::gpu_power::profile::Review>,
        graphics_mode: Option<&'a crate::graphics_mode::profile::Review>,
    },
}

impl ModeReview<'_> {
    fn background(self) -> bool {
        matches!(
            self,
            Self::Confirmed {
                permission: Some(_),
                ..
            }
        )
    }
}

fn check_background_software(
    hal: &Hal,
    settings: &ProfileSettings,
    review: ModeReview<'_>,
) -> Result<()> {
    if review.background() && settings.fan_software_curves.is_some() {
        hal.ensure_software_fan_restart_allowed()?;
    }
    Ok(())
}

fn check_mode_review(before: &Checkpoint, review: ModeReview<'_>) -> Result<()> {
    check_authority(review)?;
    if let ModeReview::Confirmed {
        permission: Some(permission),
        ..
    } = review
    {
        if before.platform.needs_system()
            || before
                .graphics
                .stages(before.mode.as_ref())?
                .is_some_and(|s| !s.requests().is_empty())
        {
            permission.require_system()?;
        }
    }
    let (mode, display, gpu_power, graphics_mode) = match review {
        ModeReview::Confirmed {
            mode,
            display,
            gpu_power,
            graphics_mode,
            ..
        } => (mode, display, gpu_power, graphics_mode),
        #[cfg(test)]
        ModeReview::Unattended => return Ok(()),
    };
    if before.mux.as_ref() != graphics_mode {
        bail!("档案显卡模式预览已变化或尚未预览，请重新读取后确认");
    }
    if before.graphics.power.as_ref() != gpu_power {
        bail!("档案独显预览已变化或尚未预览，请重新读取后确认");
    }
    if display.is_some() && before.display.as_ref() != display {
        bail!("档案显示预览已变化，请重新读取后确认");
    }
    if before.mode.as_ref().map(|plan| &plan.target) != mode {
        bail!("档案的电源模式联动已变化或尚未预览，请重新读取后确认");
    }
    Ok(())
}

#[cfg(test)]
pub(crate) fn apply_reviewed(
    hal: &Hal,
    gpu: &GpuController,
    devices: Devices<'_>,
    actual: &super::ProfileFile,
    expected: &super::ProfileFile,
    expected_mode: Option<&powerctl::mode::Target>,
    expected_power: Option<&crate::gpu_power::profile::Review>,
    permission: Option<&super::application::Permission>,
) -> Result<Vec<ApplyStep>> {
    if actual != expected {
        bail!("档案已变化，请重新预览后确认");
    }
    transact(
        hal,
        gpu,
        devices,
        &actual.settings,
        ModeReview::Confirmed {
            permission,
            mode: expected_mode,
            source: None,
            display: None,
            gpu_power: expected_power,
            graphics_mode: None,
        },
    )
    .map(|outcome| outcome.steps)
}

#[cfg(test)]
pub(crate) fn apply_settings(
    hal: &Hal,
    gpu: &GpuController,
    devices: Devices<'_>,
    settings: &ProfileSettings,
) -> Result<Vec<ApplyStep>> {
    transact(hal, gpu, devices, settings, ModeReview::Unattended).map(|outcome| outcome.steps)
}

fn stage(steps: &mut Vec<ApplyStep>, name: &str, result: Result<()>) -> Result<()> {
    let message = result
        .as_ref()
        .err()
        .map(|e| format!("{e:#}"))
        .unwrap_or_else(|| "已确认".into());
    steps.push(ApplyStep {
        step: name.into(),
        ok: result.is_ok(),
        message,
    });
    result
}

fn rollback(
    hal: &Hal,
    gpu: &GpuController,
    devices: &mut Scopes<'_>,
    before: &Checkpoint,
    touched: &mut Touched,
    steps: &mut Vec<ApplyStep>,
) {
    mux::rollback(hal, devices, &mut touched.fan, steps);
    before.graphics.rollback(gpu, devices, touched.gpu, steps);
    if touched.light_bar {
        if let Some(plan) = &before.light_bar {
            let _ = stage(steps, "恢复独立灯条", devices.light_bar.rollback(hal, plan));
        }
    }
    before
        .keyboard
        .rollback(hal, devices, &touched.keyboard, steps);
    if let Some(plan) = before.touchpad.as_ref().filter(|_| touched.touchpad) {
        let _ = stage(
            steps,
            "恢复本次触摸板状态",
            devices.touchpad.rollback(hal, plan),
        );
    }
    if let Some(plan) = before.microphones.as_ref().filter(|_| touched.microphones) {
        let _ = stage(
            steps,
            "恢复本次麦克风状态",
            devices.microphones.rollback(hal, plan),
        );
    }
    before
        .platform
        .rollback(hal, devices, &mut touched.platform, steps);
    if let Some(plan) = before
        .dust
        .as_ref()
        .filter(|plan| touched.dust && plan.restores_before_fan())
    {
        let _ = stage(steps, "恢复除尘计划", devices.dust.rollback(hal, plan));
    }
    restore_controls(hal, devices, before, touched, steps);
    if touched.fan {
        if let Some(fan) = &before.fan {
            let _ = stage(steps, "恢复风扇", fanctl::restore_checkpoint(hal, fan));
        }
    }
    if let Some(plan) = before
        .dust
        .as_ref()
        .filter(|plan| touched.dust && !plan.restores_before_fan())
    {
        let _ = stage(steps, "恢复除尘计划", devices.dust.rollback(hal, plan));
    }
}

fn restore_controls(
    hal: &Hal,
    devices: &Scopes<'_>,
    before: &Checkpoint,
    touched: &mut Touched,
    steps: &mut Vec<ApplyStep>,
) {
    if touched.saved && !before.power.is_empty() {
        let result = powerctl::restore_patches(hal, &before.power);
        let _ = stage(steps, "恢复保存的功耗与电压", result);
    }
    // OEM 模式可能改变风扇策略；先恢复模式，再还原风扇快照。
    if touched.mode {
        if let Some(mode) = &before.mode {
            let _ = stage(steps, "恢复模式 GPU 频率", devices.mode.recover_clocks());
            let _ = stage(
                steps,
                "恢复 OEM 电源模式",
                powerctl::mode::restore_checkpoint(hal, &mode.core),
            );
        }
    }
    if touched.options.dirty() {
        if let Some(plan) = &before.options {
            let _ = stage(
                steps,
                "恢复性能选项",
                plan.rollback(hal, &mut touched.options, before.fan.is_none()),
            );
        }
    }
    if touched.runtime {
        if let Some(runtime) = &before.runtime {
            let _ = stage(steps, "恢复即时 CPU", runtime.restore());
        }
    }
}

#[cfg(test)]
#[path = "apply/tests.rs"]
pub(in crate::profiles) mod tests;

fn apply_device_controls(
    hal: &Hal,
    devices: &mut Scopes<'_>,
    before: &Checkpoint,
    touched: &mut Touched,
    steps: &mut Vec<ApplyStep>,
    review: ModeReview<'_>,
) -> Result<()> {
    if let Some(plan) = &before.microphones {
        guard(steps, review)?;
        stage(
            steps,
            "麦克风静音",
            devices
                .microphones
                .execute(hal, plan, &mut touched.microphones),
        )?;
    }
    if let Some(plan) = &before.touchpad {
        guard(steps, review)?;
        stage(
            steps,
            "触摸板开关",
            devices.touchpad.execute(hal, plan, &mut touched.touchpad),
        )?;
    }
    before
        .keyboard
        .execute(hal, devices, &mut touched.keyboard, steps, review)?;
    if let Some(plan) = &before.light_bar {
        guard(steps, review)?;
        stage(
            steps,
            "独立灯条",
            devices.light_bar.execute(hal, plan, &mut touched.light_bar),
        )?;
    }
    Ok(())
}

fn check_authority(review: ModeReview<'_>) -> Result<()> {
    match review {
        #[cfg(test)]
        ModeReview::Unattended => (),
        ModeReview::Confirmed {
            permission, source, ..
        } => {
            if let Some(permission) = permission {
                permission.check()?;
            }
            if let Some((source, expected)) = source {
                source.check(expected)?;
            }
        }
    }
    Ok(())
}
fn guard(steps: &mut Vec<ApplyStep>, review: ModeReview<'_>) -> Result<()> {
    if let Err(error) = check_authority(review) {
        return stage(steps, "档案授权复核", Err(error));
    }
    Ok(())
}
