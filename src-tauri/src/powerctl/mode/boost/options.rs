use super::*;

pub(crate) struct Plan {
    pub(super) revision: String,
    checkpoint: Option<super::super::Plan>,
}
#[derive(Default)]
pub(crate) struct Change {
    saved: bool,
    fan: bool,
}
impl Change {
    pub fn dirty(&self) -> bool {
        self.saved || self.fan
    }
}
impl Plan {
    pub fn prepare(hal: &Hal, values: &Values) -> Result<Self> {
        hal.with_dchu(|io| super::super::support::ensure(io))?;
        let (driver, _) = driver_status(hal)?;
        let current = hal.with_dchu(|io| preview(io, driver))?;
        let preferences = values.preferences(current.core.preferences, current.caps)?;
        let checkpoint = if preferences == current.core.preferences {
            None
        } else {
            Some(super::super::workflow::checkpoint_options(
                hal,
                current.core.mode,
                values,
            )?)
        };
        if revision(hal)? != current.revision {
            bail!("性能选项或驱动已变化，请重新预览");
        }
        Ok(Self {
            revision: current.revision,
            checkpoint,
        })
    }
    pub fn changes_runtime(&self) -> bool {
        self.checkpoint
            .as_ref()
            .is_some_and(|plan| plan.before.core.mode == 2)
    }
    pub fn execute(&self, hal: &Hal, resume_software: bool, change: &mut Change) -> Result<()> {
        hal.ensure_mode_change_allowed()?;
        crate::fanctl::dust::ensure_manual_allowed(hal)?;
        if revision(hal)? != self.revision {
            bail!("性能选项或驱动已变化，请重新预览");
        }
        let Some(plan) = &self.checkpoint else {
            return Ok(());
        };
        plan.review(hal)?;
        let before = plan.before.core;
        let target = CoreSnapshot {
            preferences: plan.preferences,
            command: mode_command(before.mode, plan.preferences),
            ..before
        };
        plan.remember(hal)?;
        let result: Result<()> = (|| {
            if before.mode == 2 && plan.before.fan.software_curves().is_some() {
                change.fan = true;
                crate::fanctl::software::stop(hal)?;
            }
            change.saved = true;
            hal.with_dchu(|io| write_target(io, target))?;
            if before.mode == 2 && before.preferences[0] != target.preferences[0] {
                change.fan = true;
                crate::fanctl::apply_mode_policy(hal, 0, 0)?;
            }
            if before.mode == 2 && resume_software {
                if let Some(curves) = plan.before.fan.software_curves() {
                    crate::fanctl::software::apply(hal, curves)?;
                }
            }
            Ok(())
        })();
        if let Err(error) = result {
            let recovery = self.rollback(hal, change, true);
            bail!(ccc_core::msr::with_recovery(
                format!("性能选项设置失败：{error:#}"),
                recovery.map_err(|error| format!("{error:#}"))
            ));
        }
        Ok(())
    }
    pub fn rollback(&self, hal: &Hal, change: &mut Change, restore_fan: bool) -> Result<()> {
        hal.ensure_mode_change_allowed()?;
        crate::fanctl::dust::ensure_manual_allowed(hal)?;
        let Some(plan) = &self.checkpoint else {
            return Ok(());
        };
        let mut errors = Vec::new();
        if change.saved {
            match hal.with_dchu(|io| recover_options(io, plan.before.core)) {
                Ok(()) => change.saved = false,
                Err(error) => errors.push(format!("性能选项：{error:#}")),
            }
        }
        if change.fan && restore_fan {
            match crate::fanctl::restore_checkpoint(hal, &plan.before.fan) {
                Ok(()) => change.fan = false,
                Err(error) => errors.push(format!("关联风扇：{error:#}")),
            }
        }
        if !errors.is_empty() {
            bail!(errors.join("；"));
        }
        Ok(())
    }
}

fn recover_options(io: &impl ModeIo, before: CoreSnapshot) -> Result<()> {
    if byte(io, 1, 1)? != before.mode {
        bail!("当前模式已变化，拒绝旧选项恢复");
    }
    let mut errors = Vec::new();
    for (page, offset, value) in [
        (4, 8, before.preferences[0]),
        (1, 32, before.preferences[1]),
    ] {
        if let Err(error) = settings::apply(io, page, &[Patch::new(offset, [value])], None) {
            errors.push(error);
        }
    }
    if before.mode == 2 {
        if let Err(error) = io.send_mode(before.command) {
            errors.push(format!("{error:#}"));
        }
    }
    match capture(io, false) {
        Ok(current) if current == before => (),
        Ok(_) => errors.push("性能选项恢复回读不一致".into()),
        Err(error) => errors.push(format!("恢复回读：{error:#}")),
    }
    if !errors.is_empty() {
        bail!(errors.join("；"));
    }
    Ok(())
}

#[derive(Clone, Copy)]
pub(crate) enum FanTarget {
    Firmware(u8),
    Software,
}
pub(crate) fn validate_fan(mode: u8, turbo: bool, fan: FanTarget) -> Result<()> {
    if mode == 2 && turbo && matches!(fan, FanTarget::Firmware(6) | FanTarget::Software) {
        bail!("性能模式下Turbo风扇不能与自定义或软件风扇曲线同时启用");
    }
    Ok(())
}
pub(crate) fn validate_profile(
    hal: &Hal,
    values: &Values,
    mode: Option<&super::super::Plan>,
    target: Option<FanTarget>,
) -> Result<()> {
    let current = hal.with_dchu(|io| preview(io, None))?;
    let preferences = values.preferences(current.core.preferences, current.caps)?;
    let effective_mode = mode.map_or(current.core.mode, |plan| plan.target.mode);
    let fan = match target {
        Some(target) => target,
        None if mode.is_some() => FanTarget::Firmware(mode.unwrap().target.fan_mode),
        None if crate::fanctl::software::read(hal)?.is_some() => FanTarget::Software,
        None => FanTarget::Firmware(current.fan),
    };
    validate_fan(effective_mode, preferences[0] == 1, fan)
}
