//! 原厂充电保护事务；Windows读数不作为固件策略生效证据。
#[path = "battery/calibration.rs"]
pub(crate) mod calibration;
#[path = "battery/charge.rs"]
mod charge;
#[path = "battery/energy.rs"]
pub mod energy;
#[path = "battery/model.rs"]
mod model;
#[path = "battery/native.rs"]
mod native;
#[path = "battery/protocol.rs"]
mod protocol;
#[path = "battery/scope.rs"]
mod scope;
pub(crate) use scope::Scope;
#[path = "battery/profile.rs"]
mod profile;
pub use model::ChargeTarget;
pub(crate) use profile::{Change, Plan, validate_targets};
#[cfg(test)]
#[path = "battery/tests.rs"]
mod tests;
use crate::hal::Hal;
use anyhow::{Context, Result, anyhow, bail};
pub use model::{ChargeRequest, Profile};
pub(crate) use model::{ChargeState, ChargeValues, Diagnostics, PowerStatus};
use std::sync::Mutex;

#[cfg(not(test))]
pub(crate) fn system_power() -> Result<PowerStatus> {
    native::system_power()
}
#[cfg(test)]
pub(crate) fn system_power() -> Result<PowerStatus> {
    bail!("隔离测试未注入系统电源状态")
}

pub(crate) trait Io {
    fn charge(&self) -> Result<Option<ChargeState>>;
    fn send(&self, values: &ChargeValues) -> Result<()>;
    fn power(&self) -> Result<PowerStatus>;
    fn diagnostics(&self) -> Result<Option<Diagnostics>>;
}
fn read(io: &dyn Io, baseline: &Option<ChargeState>) -> Profile {
    let mut warnings = Vec::new();
    let charge = io
        .charge()
        .map_err(|e| warnings.push(format!("充电保护：{e:#}")))
        .ok()
        .flatten();
    let power = io
        .power()
        .map_err(|e| warnings.push(format!("电源状态：{e:#}")))
        .ok();
    let diagnostics = if power.as_ref().is_some_and(|p| p.present == Some(true)) {
        io.diagnostics()
            .map_err(|e| warnings.push(format!("电池诊断：{e:#}")))
            .ok()
            .flatten()
    } else {
        None
    };
    Profile {
        revision: charge.as_ref().map(ChargeState::revision),
        charge,
        power,
        diagnostics,
        can_restore: baseline.is_some(),
        warnings,
        charge_lock_reason: None,
    }
}
fn current(io: &dyn Io) -> Result<ChargeState> {
    io.charge()?.context("本机未提供已确认的充电阈值接口")
}
fn write(io: &dyn Io, target: &ChargeState) -> Result<()> {
    current(io)?.same_capabilities(target)?;
    io.send(&target.current)?;
    if current(io)? != *target {
        bail!("充电保护写后回读不一致");
    }
    Ok(())
}
fn apply(io: &dyn Io, baseline: &mut Option<ChargeState>, request: &ChargeRequest) -> Result<()> {
    if io.power()?.present != Some(true) {
        bail!("未确认电池存在，未修改充电保护");
    }
    let before = current(io)?;
    if before.revision() != request.expected_revision {
        bail!("充电状态或候选已变化，请重新读取并确认");
    }
    charge::Plan::prepare(before, baseline, &request.values)?.execute(io, baseline, &mut false)
}
fn restore(io: &dyn Io, baseline: &mut Option<ChargeState>, expected: &str) -> Result<()> {
    if io.power()?.present != Some(true) {
        bail!("未确认电池存在，未恢复充电保护");
    }
    let before = current(io)?;
    if before.revision() != expected {
        bail!("充电状态或候选已变化，请重新读取并确认恢复");
    }
    let initial = baseline.as_ref().context("没有可恢复的充电初始快照")?;
    charge::Plan::restore(before, initial)?.execute(io, baseline, &mut false)
}
pub struct Controller {
    baseline: Mutex<Option<ChargeState>>,
    energy_path: std::path::PathBuf,
    calibration: Mutex<Option<calibration::Client>>,
    #[cfg(test)]
    io: Option<std::sync::Arc<dyn Io + Send + Sync>>,
    #[cfg(test)]
    energy_io: Option<std::sync::Arc<dyn energy::Io + Send + Sync>>,
}
impl Controller {
    pub fn new(energy_path: std::path::PathBuf) -> Self {
        Self {
            baseline: Mutex::new(None),
            energy_path,
            calibration: Mutex::new(None),
            #[cfg(test)]
            io: None,
            #[cfg(test)]
            energy_io: None,
        }
    }
    pub fn read(&self, hal: &Hal) -> Result<Profile> {
        let scope = self.scope()?;
        let _write = hal.begin_write()?;
        self.with_io(hal, |io, energy| {
            let mut profile = read(io, &scope.baseline);
            profile.charge_lock_reason = match scope.charge_lock(energy).and_then(|reason| {
                scope.ensure_available()?;
                Ok(reason)
            }) {
                Ok(reason) => reason.map(String::from),
                Err(error) => {
                    profile.warnings.push(format!("电池控制记录：{error:#}"));
                    Some(format!("充电写入暂停：{error:#}"))
                }
            };
            Ok(profile)
        })
    }
    pub fn apply(&self, hal: &Hal, request: &ChargeRequest) -> Result<()> {
        let mut scope = self.scope()?;
        let _write = hal.begin_write()?;
        self.with_io(hal, |io, energy| {
            scope.ensure_charge_available(energy)?;
            apply(io, &mut scope.baseline, request)
        })
    }
    pub fn restore(&self, hal: &Hal, expected: &str) -> Result<()> {
        let mut scope = self.scope()?;
        let _write = hal.begin_write()?;
        self.with_io(hal, |io, energy| {
            scope.ensure_charge_available(energy)?;
            restore(io, &mut scope.baseline, expected)
        })
    }
}
