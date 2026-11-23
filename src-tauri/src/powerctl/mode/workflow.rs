use super::*;
use crate::fanctl;
mod storage;
pub(crate) use storage::{RecoveryPreview, StoredModeSnapshot};

#[derive(Clone, PartialEq)]
pub struct Snapshot {
    pub(super) core: CoreSnapshot,
    pub(super) fan: fanctl::FanCheckpoint,
    system: Option<system::Snapshot>,
    pub(super) energy: Option<energy::State>,
    pub(super) links: Option<links::Initial>,
}
pub struct Plan {
    pub target: Target,
    pub(super) before: Snapshot,
    pub(super) preferences: [u8; 2],
    pub(super) options: Option<boost::Values>,
    pub(super) options_revision: Option<String>,
    entry: Entry,
    attempted: std::cell::Cell<bool>,
}
#[derive(Clone, Copy)]
enum Entry {
    Selection,
    Supply { activate_plan: bool },
    Options { fan: Option<u8> },
}
impl Entry {
    fn remembers_ac(self) -> bool {
        matches!(self, Self::Selection)
    }
    fn activates_plan(self) -> bool {
        match self {
            Self::Supply { activate_plan } => activate_plan,
            _ => true,
        }
    }
}
#[derive(Default)]
struct Progress {
    wrote: bool,
    effects_started: bool,
}

#[cfg(test)]
pub fn checkpoint(hal: &Hal, wanted: u8) -> Result<Plan> {
    checkpoint_for_profile(hal, wanted, None)
}
pub(crate) fn checkpoint_for_profile(
    hal: &Hal,
    wanted: u8,
    options: Option<&boost::Values>,
) -> Result<Plan> {
    prepare(hal, wanted, options, Entry::Selection)
}
pub(crate) fn checkpoint_for_supply(hal: &Hal, wanted: u8, activate_plan: bool) -> Result<Plan> {
    prepare(hal, wanted, None, Entry::Supply { activate_plan })
}
pub(super) fn checkpoint_options(
    hal: &Hal,
    wanted: u8,
    values: &boost::Values,
    fan: Option<u8>,
) -> Result<Plan> {
    prepare(hal, wanted, Some(values), Entry::Options { fan })
}
fn prepare(hal: &Hal, wanted: u8, options: Option<&boost::Values>, entry: Entry) -> Result<Plan> {
    hal.ensure_mode_change_allowed()?;
    fanctl::dust::ensure_manual_allowed(hal)?;
    let (core, target, preferences, energy) = configured(hal, wanted, options, entry)?;
    if target.dtt
        && (!matches!(entry, Entry::Options { .. })
            || preferences != core.preferences
            || target.command() != core.command)
    {
        boost::require_driver(hal)?;
    }
    let options_revision = options.map(|_| boost::revision(hal)).transpose()?;
    let fan_profile = fanctl::read_profile(hal)?;
    if !fan_profile.available_modes.contains(&target.fan_mode) {
        bail!("原厂分模式风扇策略未被当前固件声明支持");
    }
    if target.fan_mode == 6 && fan_profile.curve_kind == fanctl::CurveKind::TwoPoint {
        fanctl::validate_curve(hal, [None; 3])?;
    }
    let fan = fan_checkpoint(hal, core.mode, entry)?;
    let system = system::capture(hal.system_power.as_ref(), &target)?;
    Ok(Plan {
        target,
        before: Snapshot {
            core,
            fan,
            system,
            energy,
            links: None,
        },
        preferences,
        options: options.cloned(),
        options_revision,
        entry,
        attempted: std::cell::Cell::new(false),
    })
}

fn fan_checkpoint(hal: &Hal, mode: u8, entry: Entry) -> Result<fanctl::FanCheckpoint> {
    let mut checkpoint = fanctl::checkpoint(hal, true)?;
    if mode == 3 && matches!(entry, Entry::Options { fan: Some(_) }) {
        let quiet = hal.with_dchu(|io| {
            let caps = io.read_bios_capabilities(0, 22)?;
            fanctl::supports_noiseless(&caps)
                .then(|| fanctl::saved::entertainment_quiet(io))
                .transpose()
        })?;
        if let Some(enabled) = quiet {
            checkpoint.remember_entertainment_quiet(enabled);
        }
    }
    Ok(checkpoint)
}

impl Plan {
    pub(super) fn remember(&self, hal: &Hal) -> Result<()> {
        let mut recovery = hal.recovery()?;
        let initial = recovery.oem_mode.get_or_insert_with(|| self.before.clone());
        if initial.core.ac_mode.is_none() {
            initial.core.ac_mode = self.before.core.ac_mode;
        }
        if initial.energy.is_none() {
            initial.energy = self.before.energy;
        }
        if let Some(enabled) = self.before.fan.entertainment_quiet() {
            initial.fan.remember_entertainment_quiet(enabled);
        }
        if let Some(before) = &self.before.system {
            if let Some(system) = &mut initial.system {
                system.merge(before);
            } else {
                initial.system = Some(before.clone());
            }
        }
        Ok(())
    }
    pub(super) fn review(&self, hal: &Hal) -> Result<()> {
        let (core, target, preferences, energy) =
            configured(hal, self.target.mode, self.options.as_ref(), self.entry)?;
        if core != self.before.core
            || target != self.target
            || preferences != self.preferences
            || energy != self.before.energy
        {
            bail!("原厂模式、选项或联动已变化，请重新预览");
        }
        if let Some(expected) = &self.options_revision {
            if boost::revision(hal)? != *expected {
                bail!("性能选项或驱动已变化，请重新预览");
            }
        }
        if self.target.dtt
            && (!matches!(self.entry, Entry::Options { .. })
                || preferences != core.preferences
                || target.command() != core.command)
        {
            boost::require_driver(hal)?;
        }
        if fan_checkpoint(hal, core.mode, self.entry)? != self.before.fan {
            bail!("模式恢复所需的风扇状态已变化，请重新预览");
        }
        system::review(hal.system_power.as_ref(), &self.before.system, &self.target)
    }
    #[cfg(test)]
    pub fn execute(&self, hal: &Hal) -> Result<()> {
        self.execute_guarded(hal, &mut || Ok(()))
    }
    #[cfg(test)]
    pub(crate) fn execute_guarded(
        &self,
        hal: &Hal,
        permit: &mut dyn FnMut() -> Result<()>,
    ) -> Result<()> {
        self.execute_linked(
            hal,
            permit,
            &mut |_| Ok(()),
            &mut |_| Ok(()),
            &mut || Ok(()),
        )
    }
    // 调用方许可可以读取设备，必须在 DCHU 锁外执行。
    pub(crate) fn execute_linked(
        &self,
        hal: &Hal,
        permit: &mut dyn FnMut() -> Result<()>,
        after_mode: &mut dyn FnMut(&mut dyn FnMut() -> Result<()>) -> Result<()>,
        after_energy: &mut dyn FnMut(&mut dyn FnMut() -> Result<()>) -> Result<()>,
        compensate: &mut dyn FnMut() -> Result<()>,
    ) -> Result<()> {
        self.reset_attempted();
        hal.ensure_mode_change_allowed()?;
        fanctl::dust::ensure_manual_allowed(hal)?;
        self.review(hal)?;
        permit()?;
        let previous_baseline = hal.recovery()?.oem_mode.clone();
        self.remember(hal)?;
        let mut progress = Progress::default();
        let result = self.execute_stages(hal, permit, after_mode, after_energy, &mut progress);
        self.attempted.set(progress.wrote);
        if let Err(error) = result {
            let mut errors = Vec::new();
            if progress.effects_started {
                if let Err(error) = compensate() {
                    errors.push(format!("模式关联设置：{error:#}"));
                }
            }
            let restored = if progress.wrote {
                recover_snapshot(hal, &self.before).map_err(|e| format!("{e:#}"))
            } else {
                hal.recovery()
                    .map(|mut state| state.oem_mode = previous_baseline)
                    .map_err(|e| format!("{e:#}"))
            };
            if let Err(error) = restored {
                errors.push(error);
            }
            let recovery = if errors.is_empty() {
                Ok(())
            } else {
                Err(errors.join("；"))
            };
            bail!(ccc_core::msr::with_recovery(format!("{error:#}"), recovery));
        }
        Ok(())
    }
    pub(crate) fn attempted(&self) -> bool {
        self.attempted.get()
    }
    pub(super) fn reset_attempted(&self) {
        self.attempted.set(false);
    }
    fn execute_stages(
        &self,
        hal: &Hal,
        permit: &mut dyn FnMut() -> Result<()>,
        after_mode: &mut dyn FnMut(&mut dyn FnMut() -> Result<()>) -> Result<()>,
        after_energy: &mut dyn FnMut(&mut dyn FnMut() -> Result<()>) -> Result<()>,
        progress: &mut Progress,
    ) -> Result<()> {
        permit()?;
        progress.wrote = self.before.fan.software_curves().is_some();
        fanctl::software::stop_safely(hal)?;
        if self.preferences != self.before.core.preferences {
            permit()?;
            hal.with_dchu(|io| {
                progress.wrote = true;
                boost::write_saved(io, self.preferences)
            })?;
        }
        write_core(hal, &self.target, permit, &mut progress.wrote)?;
        permit()?;
        progress.effects_started = true;
        after_mode(permit)?;
        energy::apply(hal, self.before.energy, self.target.energy, permit)?;
        after_energy(permit)?;
        system::apply(
            hal.system_power.as_ref(),
            &self.before.system,
            &self.target,
            permit,
        )?;
        permit()?;
        fanctl::apply_mode_policy(hal, self.target.fan_saved_mode, self.target.fan_mode)?;
        permit()?;
        fanctl::set_mode_offset(hal, u32::from(self.target.fan_offset_pct))
    }
    pub fn restore(&self, hal: &Hal) -> Result<()> {
        recover_snapshot(hal, &self.before)
    }
}

#[cfg(test)]
#[path = "workflow/ac_tests.rs"]
mod ac_tests;
#[cfg(test)]
#[path = "workflow/energy_tests.rs"]
mod energy_tests;
#[cfg(test)]
#[path = "workflow/linked_tests.rs"]
mod linked_tests;

fn configured(
    hal: &Hal,
    wanted: u8,
    options: Option<&boost::Values>,
    entry: Entry,
) -> Result<(CoreSnapshot, Target, [u8; 2], Option<energy::State>)> {
    hal.with_dchu(|io| {
        let core = capture(io, entry.remembers_ac())?;
        let energy = if matches!(entry, Entry::Options { .. }) {
            None
        } else {
            Some(energy::read(io, hal.reviewed_dsdt)?)
        };
        let mut target = policy::target_for(io, wanted, entry.activates_plan(), energy)?;
        target.ac_mode = entry.remembers_ac().then_some(wanted);
        let preferences = boost::configure(io, core, &mut target, options)?;
        if let Entry::Options { fan: Some(fan) } = entry {
            target.fan_mode = fan;
            target.fan_saved_mode = if wanted == 3 {
                fanctl::FAN_MODE_AUTO
            } else {
                fan
            };
        }
        Ok((core, target, preferences, energy))
    })
}

pub(super) fn restore_snapshot(hal: &Hal, before: &Snapshot) -> Result<()> {
    energy::review_restore(hal, before.energy)?;
    recover_snapshot(hal, before)
}
fn recover_snapshot(hal: &Hal, before: &Snapshot) -> Result<()> {
    hal.ensure_mode_change_allowed()?;
    fanctl::dust::ensure_manual_allowed(hal)?;
    let current_energy = energy::review_restore(hal, before.energy);
    // 模式先恢复，风扇快照最后恢复；系统恢复的失败不跳过其他域。
    let mut errors = Vec::new();
    let core = boost::prepare_recovery(hal, before.core)
        .and_then(|()| hal.with_dchu(|io| recover(io, before.core)));
    for (name, result) in [
        ("OEM 模式", core),
        (
            "EnergyStar",
            current_energy
                .and_then(|current| energy::apply(hal, current, before.energy, &mut || Ok(()))),
        ),
        (
            "Windows 电源策略",
            system::restore(hal.system_power.as_ref(), &before.system),
        ),
        ("关联风扇", fanctl::restore_checkpoint(hal, &before.fan)),
    ] {
        if let Err(error) = result {
            errors.push(format!("{name}：{error:#}"));
        }
    }
    if !errors.is_empty() {
        bail!(errors.join("；"));
    }
    Ok(())
}
#[cfg(test)]
pub fn apply_preview(hal: &Hal, expected: &Target) -> Result<()> {
    let plan = checkpoint(hal, expected.mode)?;
    if plan.target != *expected {
        bail!("模式联动已变化，请重新读取并确认");
    }
    plan.execute(hal)
}
pub fn restore_checkpoint(hal: &Hal, plan: &Plan) -> Result<()> {
    plan.restore(hal)
}
