use super::*;

pub(crate) struct Plan {
    revision: String,
    checkpoint: Option<super::super::Plan>,
    runtime: bool,
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
    pub fn prepare(hal: &Hal, values: &Values, profile_fan: Option<FanTarget>) -> Result<Self> {
        values.validate()?;
        hal.with_dchu(|io| super::super::support::ensure(io))?;
        let (driver, _) = driver_status(hal)?;
        let current = hal.with_dchu(|io| preview(io, driver))?;
        let preferences = values.preferences(current.core.preferences, current.caps)?;
        let command = option_command(current.core.mode, preferences, current.runtime_options);
        if command & 0x40 != 0 {
            let fan = match profile_fan {
                Some(fan) => fan,
                None if fanctl::software::active(hal)? => FanTarget::Software,
                None => FanTarget::Firmware(current.fan),
            };
            validate_active_fan(true, fan)?;
        }
        let fan = current.fan_target(command, None)?;
        let target = CoreSnapshot {
            preferences,
            command,
            ..current.core
        };
        Self::from_preview(hal, current, target, fan, true)
    }
    pub(super) fn from_preview(
        hal: &Hal,
        current: Preview,
        target: CoreSnapshot,
        fan: Option<u8>,
        remember_quiet: bool,
    ) -> Result<Self> {
        let checkpoint = if target == current.core && fan.is_none() {
            None
        } else {
            Some(super::super::workflow::checkpoint_options(
                hal,
                target,
                fan,
                remember_quiet,
            )?)
        };
        if revision(hal)? != current.revision {
            bail!("性能选项或驱动已变化，请重新预览");
        }
        Ok(Self {
            revision: current.revision,
            checkpoint,
            runtime: current.runtime_options != [false; 2],
        })
    }
    pub fn changes_runtime(&self) -> bool {
        self.checkpoint.as_ref().is_some_and(|plan| self.runtime || plan.option_fan().is_some())
    }
    pub fn execute(&self, hal: &Hal, resume_software: bool, change: &mut Change) -> Result<()> {
        hal.ensure_mode_change_allowed()?;
        fanctl::ensure_manual_allowed(hal)?;
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
            command: plan.target.command(),
            ..before
        };
        plan.remember(hal)?;
        let result: Result<()> = (|| {
            if self.runtime && plan.before.fan.software_curves().is_some() {
                change.fan = true;
                crate::fanctl::software::stop_safely(hal)?;
            }
            if target.preferences != before.preferences || target.command != before.command {
                if target.command & 0x40 != 0 {
                    fanctl::handoff_for_turbo(hal, &mut change.fan)?;
                }
                change.saved = true;
                hal.with_dchu(|io| write_target(io, target))?;
            }
            if let Some(fan) = plan.option_fan() {
                change.fan = true;
                fanctl::apply_mode_policy(hal, fan)?;
                if plan.before.fan.entertainment_quiet().is_some() {
                    let quiet = Patch::new(
                        fanctl::saved::ENTERTAINMENT_QUIET_OFFSET,
                        [u8::from(fan.mode() == Some(FAN_MODE_NOISELESS))],
                    );
                    hal.with_dchu(|io| {
                        settings::apply(io, 4, &[quiet], None).map_err(|e| anyhow!(e))
                    })?;
                }
            }
            if self.runtime && resume_software {
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
        fanctl::ensure_manual_allowed(hal)?;
        let Some(plan) = &self.checkpoint else {
            return Ok(());
        };
        let mut errors = Vec::new();
        if change.saved {
            let result = (|| {
                hal.with_dchu(|io| check_recover_mode(io, plan.before.core))?;
                prepare_recovery(hal, plan.before.core, &mut change.fan)?;
                hal.with_dchu(|io| recover_options(io, plan.before.core))
            })();
            match result {
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

fn check_recover_mode(io: &impl ModeIo, before: CoreSnapshot) -> Result<()> {
    if current_mode(io)? != before.mode {
        bail!("当前模式已变化，拒绝旧选项恢复");
    }
    Ok(())
}

fn recover_options(io: &impl ModeIo, before: CoreSnapshot) -> Result<()> {
    check_recover_mode(io, before)?;
    validate_entertainment_command(io, before.command)?;
    let mut errors = Vec::new();
    for (page, offset, value) in [
        (4, 8, before.preferences[0]),
        (1, 32, before.preferences[1]),
    ] {
        if let Err(error) = settings::apply(io, page, &[Patch::new(offset, [value])], None) {
            errors.push(error);
        }
    }
    if runtime_options(io, before.mode)? != [false; 2] {
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
pub(crate) fn ensure_fan_allowed(hal: &Hal, fan: FanTarget) -> Result<()> {
    hal.with_dchu(|io| ensure_fan_allowed_on(io, fan))
}

pub(crate) fn ensure_fan_allowed_on(io: &impl ModeIo, fan: FanTarget) -> Result<()> {
    if matches!(
        fan,
        FanTarget::Firmware(FAN_MODE_CUSTOM | FAN_MODE_NOISELESS) | FanTarget::Software
    ) {
        validate_active_fan(turbo_enabled(io, current_mode(io)?)?, fan)?;
    }
    Ok(())
}

pub(crate) fn validate_active_fan(turbo: bool, fan: FanTarget) -> Result<()> {
    if turbo && matches!(fan, FanTarget::Firmware(FAN_MODE_NOISELESS)) {
        bail!("Turbo 风扇与{NOISELESS_FAN_LABEL}不能同时开启");
    }
    if turbo
        && matches!(
            fan,
            FanTarget::Firmware(FAN_MODE_CUSTOM) | FanTarget::Software
        )
    {
        bail!("Turbo 风扇不能与自定义固件风扇曲线或软件风扇曲线同时启用");
    }
    Ok(())
}
