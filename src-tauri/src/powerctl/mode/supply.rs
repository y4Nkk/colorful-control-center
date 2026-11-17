//! 原厂供电模式的设备事实与完整联动预览。
use super::{ModeProfile, Target, energy};
use crate::gpuctl::turbo::mode as clocks;
use crate::hal::{Hal, oem_identity::Model};
use anyhow::{Result, anyhow, bail};
use ccc_core::oem_supply_mode::{self, Trigger};
use serde::Serialize;
use sha2::{Digest, Sha256};
mod control;
#[cfg(not(control_sim))]
mod controller;
pub(super) mod gpu;
mod native;
pub(super) mod plans;
pub(super) mod record;
#[cfg(all(not(test), not(control_sim)))]
mod runtime;
mod scheduler;
mod transaction;
#[cfg(not(control_sim))]
pub(crate) use controller::{Controller, Status};
pub(crate) use transaction::Applied;
pub(crate) const RECORD_NAME: &str = "oem-supply-mode.json";

pub(crate) fn ensure_inactive(path: &std::path::Path) -> Result<()> {
    if record::FileStore(path.to_path_buf())
        .load()?
        .is_some_and(|record| record.enabled)
    {
        bail!("原厂供电模式策略仍已启用，请先恢复该策略再启用自动档案");
    }
    Ok(())
}
#[cfg(test)]
mod transaction_tests;

#[derive(Clone)]
struct Evidence {
    model: Model,
    quiet_plus_default: bool,
    ac_mode: u8,
    current_mode: u8,
    available: Vec<u8>,
    targets: Vec<Target>,
    on_ac: Option<bool>,
    energy: energy::State,
    gpu_supported: bool,
    gpu_devices: Vec<crate::gpu_power::Device>,
    clocks: Option<clocks::Policy>,
}
#[derive(Clone, Copy)]
pub(crate) enum Scope {
    All,
    Event(Trigger),
}
impl Scope {
    pub(super) fn skips_gpu(self) -> bool {
        matches!(self, Self::Event(Trigger::Resume))
    }
    fn clock_index(self, on_ac: Option<bool>) -> Option<usize> {
        let source = if on_ac? { 0 } else { 1 };
        match self {
            Self::All => None,
            Self::Event(Trigger::SupplyChange) => Some(source),
            Self::Event(Trigger::Startup) => Some(source + 2),
            Self::Event(Trigger::Resume) => Some(source + 4),
        }
    }
    pub(super) fn clock_modes(self, review: &Review) -> [Option<u8>; 6] {
        let mut modes = review
            .conditions()
            .map(|target| target.as_ref().map(|target| target.mode));
        if let Self::Event(_) = self {
            let selected = self.clock_index(review.on_ac);
            for (index, mode) in modes.iter_mut().enumerate() {
                if Some(index) != selected {
                    *mode = None;
                }
            }
        }
        modes
    }
}
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Review {
    pub device_identity: String,
    pub identity: String,
    pub revision: String,
    pub current_mode: u8,
    pub ac_mode: u8,
    pub on_ac: Option<bool>,
    pub ac: Option<Target>,
    pub battery: Option<Target>,
    pub startup_ac: Option<Target>,
    pub startup_battery: Option<Target>,
    pub resume_ac: Option<Target>,
    pub resume_battery: Option<Target>,
    pub targets: Vec<Target>,
    pub software_fan_active: bool,
    pub current_energy: energy::State,
    pub dc_mode_power: bool,
    pub gpu: gpu::Review,
    pub clocks: Option<clocks::Review>,
}
impl Review {
    pub(super) fn unavailable_reason(&self) -> Option<&str> {
        self.unavailable_for(Scope::All)
    }
    pub(super) fn unavailable_for(&self, scope: Scope) -> Option<&str> {
        if let Scope::Event(trigger) = scope {
            let on_ac = match self.on_ac {
                Some(value) => value,
                None => return Some("供电状态未知"),
            };
            return self
                .gpu
                .candidate(trigger, on_ac)
                .unavailable_reason
                .as_deref()
                .or_else(|| self.clock_issue(scope));
        }
        self.clock_issue(scope).or_else(|| {
            self.gpu
                .candidates()
                .into_iter()
                .find_map(|candidate| candidate.unavailable_reason.as_deref())
        })
    }
    fn clock_issue(&self, scope: Scope) -> Option<&str> {
        if scope.skips_gpu() {
            return None;
        }
        let Some(clocks) = &self.clocks else {
            return Some("GPU调频预览缺失");
        };
        if let Some(index) = scope.clock_index(self.on_ac) {
            clocks
                .driver_error
                .as_deref()
                .or(clocks.candidates()[index].unavailable_reason.as_deref())
        } else {
            clocks.unavailable_reason()
        }
    }
    pub(super) fn check_clocks(
        &self,
        scope: Scope,
        approval: &record::ClockApproval,
    ) -> Result<()> {
        if scope.skips_gpu() {
            return Ok(());
        }
        let clocks = self
            .clocks
            .as_ref()
            .ok_or_else(|| anyhow!("GPU调频预览缺失"))?;
        let needed = match scope.clock_index(self.on_ac) {
            Some(index) => clocks.candidates()[index]
                .decision
                .as_ref()
                .is_some_and(|decision| decision.target.is_some()),
            None => true,
        };
        if needed {
            approval.check_review(clocks)?;
        }
        Ok(())
    }
    pub(super) fn conditions(&self) -> [&Option<Target>; 6] {
        [
            &self.ac,
            &self.battery,
            &self.startup_ac,
            &self.startup_battery,
            &self.resume_ac,
            &self.resume_battery,
        ]
    }
    pub(super) fn target(&self, trigger: Trigger, on_ac: bool) -> Option<&Target> {
        match (trigger, on_ac) {
            (Trigger::SupplyChange, true) => &self.ac,
            (Trigger::SupplyChange, false) => &self.battery,
            (Trigger::Startup, true) => &self.startup_ac,
            (Trigger::Startup, false) => &self.startup_battery,
            (Trigger::Resume, true) => &self.resume_ac,
            (Trigger::Resume, false) => &self.resume_battery,
        }
        .as_ref()
    }
}
fn review(evidence: &Evidence, software_fan_active: bool) -> Result<Option<Review>> {
    let mut normalized = evidence.clone();
    for target in &mut normalized.targets {
        target.ac_mode = None;
    }
    let evidence = &normalized;
    if !evidence.model.dc_mode_power && evidence.model.customer != 22 {
        return Ok(None);
    }
    if evidence.model.product == 0 || evidence.model.product > u16::MAX.into() {
        bail!("原厂机型标识未知，无法预览供电模式");
    }
    if !evidence.available.contains(&evidence.current_mode) {
        bail!("当前电源模式不在固件能力中");
    }
    evidence.energy.validate()?;
    let [
        ac,
        battery,
        startup_ac,
        startup_battery,
        resume_ac,
        resume_battery,
    ] = condition_targets(evidence)?;
    let targets = [
        &ac,
        &battery,
        &startup_ac,
        &startup_battery,
        &resume_ac,
        &resume_battery,
    ];
    validate_energy(evidence.energy, targets)?;
    let gpu = gpu::review(
        evidence.gpu_supported,
        &evidence.gpu_devices,
        evidence.model.dc_mode_power,
        targets.map(|target| target.as_ref().map(|target| target.mode)),
    )?;
    let clocks = evidence.clocks.clone().map(|policy| {
        clocks::review(
            policy,
            targets.map(|target| target.as_ref().map(|target| target.mode)),
        )
    });
    let device_identity = device_digest(
        &evidence.model,
        evidence.quiet_plus_default,
        &evidence.available,
    )?;
    let identity = stable_identity(evidence, &device_identity, &gpu, targets)?;
    let mut view = Review {
        device_identity,
        identity,
        revision: String::new(),
        current_mode: evidence.current_mode,
        ac_mode: evidence.ac_mode,
        on_ac: evidence.on_ac,
        ac,
        battery,
        startup_ac,
        startup_battery,
        resume_ac,
        resume_battery,
        targets: evidence.targets.clone(),
        software_fan_active,
        current_energy: evidence.energy,
        dc_mode_power: evidence.model.dc_mode_power,
        gpu,
        clocks,
    };
    view.revision = digest(&view)?;
    Ok(Some(view))
}
fn condition_targets(evidence: &Evidence) -> Result<[Option<Target>; 6]> {
    Ok([
        choose(evidence, true, Trigger::SupplyChange)?,
        choose(evidence, false, Trigger::SupplyChange)?,
        choose(evidence, true, Trigger::Startup)?,
        choose(evidence, false, Trigger::Startup)?,
        choose(evidence, true, Trigger::Resume)?,
        choose(evidence, false, Trigger::Resume)?,
    ])
}
fn stable_identity(
    evidence: &Evidence,
    device: &str,
    gpu: &gpu::Review,
    targets: [&Option<Target>; 6],
) -> Result<String> {
    let [ac, battery, startup_ac, startup_dc, resume_ac, resume_dc] = targets;
    digest(&(
        "oem-supply-mode-2",
        device,
        evidence.ac_mode,
        ac,
        battery,
        startup_ac,
        startup_dc,
        resume_ac,
        resume_dc,
        &evidence.targets,
        evidence.energy.hardware_p_state,
        gpu.identity(),
    ))
}
fn validate_energy(state: energy::State, targets: [&Option<Target>; 6]) -> Result<()> {
    for target in targets.into_iter().flatten() {
        if target.energy != Some(energy::desired(target.mode, state)?) {
            bail!("模式与固件节能目标不一致");
        }
    }
    Ok(())
}
fn choose(evidence: &Evidence, on_ac: bool, trigger: Trigger) -> Result<Option<Target>> {
    let mode = oem_supply_mode::target(oem_supply_mode::Facts {
        product: evidence.model.product,
        customer: evidence.model.customer,
        dc_mode_power: evidence.model.dc_mode_power,
        quiet_plus_default: evidence.quiet_plus_default,
        ac_mode: evidence.ac_mode,
        available: &evidence.available,
        on_ac: Some(on_ac),
        trigger,
    })
    .map_err(anyhow::Error::msg)?;
    mode.map(|mode| {
        evidence
            .targets
            .iter()
            .find(|target| target.mode == mode)
            .cloned()
            .ok_or_else(|| anyhow!("原厂供电模式联动目标缺失"))
            .map(|mut target| {
                if trigger != Trigger::SupplyChange {
                    target.power_plan = None;
                }
                target
            })
    })
    .transpose()
}
fn digest(value: &impl Serialize) -> Result<String> {
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(value)?)))
}
fn device_digest(model: &Model, quiet: bool, available: &[u8]) -> Result<String> {
    digest(&(
        "oem-supply-device-1",
        model.product,
        model.customer,
        model.dc_mode_power,
        quiet,
        available,
    ))
}
pub(crate) fn device_identity(hal: &Hal) -> Result<String> {
    hal.with_dchu(|io| {
        let model = crate::hal::oem_identity::read(io)?;
        let caps = io.read_bios_capabilities(0, 18)?;
        if caps[..2] != [0, 0] && caps[..2] != [1, 0] {
            bail!("原厂模式能力版本未知");
        }
        let quiet = caps[..2] == [1, 0] && caps[17] & 0x80 != 0;
        device_digest(&model, quiet, &super::available(io)?)
    })
}
pub(crate) fn read(
    hal: &Hal,
    gpu_devices: Vec<crate::gpu_power::Device>,
    scope: Scope,
) -> Result<Option<Review>> {
    let (model, quiet_plus_default, ac_mode, gpu_supported) = hal.with_dchu(|io| {
        let model = crate::hal::oem_identity::read(io)?;
        let caps = io.read_bios_capabilities(0, 18)?;
        if caps[..2] != [0, 0] && caps[..2] != [1, 0] {
            bail!("原厂模式能力版本未知");
        }
        let quiet = caps[..2] == [1, 0] && caps[17] & 0x80 != 0;
        let ac_mode = io.read_settings(1, 16, 1)?[0];
        let gpu_supported =
            crate::gpu_power::native::protocol::supported(&io.read_bios_capabilities(0, 20)?)?;
        Ok((model, quiet, ac_mode, gpu_supported))
    })?;
    if !model.dc_mode_power && model.customer != 22 {
        return Ok(None);
    }
    let clocks = if scope.skips_gpu() {
        None
    } else {
        Some(clocks::read(hal, &model)?)
    };
    let ModeProfile {
        current_mode,
        available_modes,
        targets,
        software_active,
        blocked_reason,
        energy,
        ..
    } = super::read(hal)?;
    let energy = energy.ok_or_else(|| anyhow!("供电模式缺少可确认的EnergyStar前态"))?;
    if let Some(reason) = blocked_reason {
        bail!("供电模式不可接管：{reason}");
    }
    review(
        &Evidence {
            model,
            quiet_plus_default,
            ac_mode,
            current_mode,
            available: available_modes,
            targets,
            on_ac: crate::battery::system_power()?.ac_online,
            energy,
            gpu_supported,
            gpu_devices,
            clocks,
        },
        software_active,
    )
}

#[cfg(test)]
mod tests;
