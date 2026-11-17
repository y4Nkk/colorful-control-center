#[path = "energy/calibration.rs"]
mod calibration;
#[path = "energy/codec.rs"]
mod codec;
#[path = "energy/model.rs"]
mod model;
#[path = "energy/native.rs"]
pub(super) mod native;
#[path = "energy/original.rs"]
mod original;
#[path = "energy/store.rs"]
mod store;
#[path = "energy/transaction.rs"]
mod transaction;
pub(crate) use transaction::{Change, Plan};
#[cfg(test)]
#[path = "energy/tests.rs"]
mod tests;
use super::{Controller, model::ChargeState};
use crate::hal::Hal;
use anyhow::{Context, Result, bail};
pub(super) use calibration::{capture as capture_calibration, restore as restore_calibration};
pub(super) use model::Snapshot as EnergySnapshot;
use model::*;
pub(crate) use model::{Capability, Clock, Policy};
pub use model::{Profile, Request, Target};
use sha2::{Digest, Sha256};
pub(super) use store::{FileStore, Store};

pub(crate) trait Io {
    fn capability(&self) -> Result<Option<Capability>>;
    fn charge(&self) -> Result<ChargeState>;
    fn present(&self) -> Result<bool>;
    fn original(&self, cap: &Capability) -> Result<Option<Policy>>;
    fn clock(&self) -> Clock;
    fn send(&self, command: u32) -> Result<()>;
    fn restore_charge(&self, charge: &ChargeState) -> Result<()>;
}
pub(super) fn observe(
    io: &dyn Io,
    store: &dyn Store,
) -> Result<Option<(Snapshot, Option<Record>)>> {
    let Some(capability) = io.capability()? else {
        return Ok(None);
    };
    let charge = io.charge()?;
    let record = store.load()?;
    let policy = match &record {
        Some(record) => {
            if record.capability != capability {
                bail!("节能能力或自动阈值已变化，未采用旧记录");
            }
            record.policy.clone()
        }
        None => io.original(&capability)?,
    };
    if let Some(policy) = &policy {
        policy.validate(&capability)?;
    }
    Ok(Some((
        Snapshot {
            policy,
            charge,
            capability,
        },
        record,
    )))
}
fn revision(before: &Snapshot, record: &Option<Record>) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&(before, record)).expect("固定节能结构"))
    )
}
fn read(io: &dyn Io, store: &dyn Store) -> Result<Option<Profile>> {
    let Some((before, record)) = observe(io, store)? else {
        return Ok(None);
    };
    Ok(Some(Profile {
        revision: revision(&before, &record),
        saved: before.policy.clone(),
        capability: before.capability,
        source: if record.is_some() {
            "application"
        } else if before.policy.is_some() {
            "oemSaved"
        } else {
            "unconfigured"
        },
        can_restore: record.is_some(),
        recovery_required: record.as_ref().is_some_and(|r| r.pending.is_some()),
        charge_reported: before.charge.current.clone(),
        has_initial_plan: record
            .as_ref()
            .map_or(before.policy.is_some(), |r| r.baseline.policy.is_some()),
        initial_charge: record.as_ref().map_or_else(
            || before.charge.current.clone(),
            |r| r.baseline.charge.current.clone(),
        ),
        charge_on_stop: record
            .as_ref()
            .map_or(before.charge, |r| r.stop_charge.clone()),
        error: record.and_then(|r| r.last_error),
        can_write: io.present()?,
    }))
}
fn emit(io: &dyn Io, target: &Policy, cap: &Capability) -> Result<()> {
    for command in codec::commands(target, cap, io.clock())? {
        io.send(command)?;
    }
    Ok(())
}
fn recover(io: &dyn Io, snapshot: &Snapshot) -> Result<()> {
    if !io.present()? || io.capability()?.as_ref() != Some(&snapshot.capability) {
        bail!("电池或节能能力已变化，保留恢复记录");
    }
    io.charge()?.same_capabilities(&snapshot.charge)?;
    let mut errors = Vec::new();
    let restored = match &snapshot.policy {
        Some(policy) => emit(io, policy, &snapshot.capability),
        None => clear_slots(io),
    };
    if let Err(error) = restored {
        errors.push(format!("节能计划：{error:#}"));
        if snapshot.policy.is_some() {
            if let Err(error) = clear_slots(io) {
                errors.push(format!("释放残留时段：{error:#}"));
            }
        }
    }
    if let Err(error) = io.restore_charge(&snapshot.charge) {
        errors.push(format!("充电策略：{error:#}"));
    }
    if !errors.is_empty() {
        bail!(errors.join("；"));
    }
    Ok(())
}
fn clear_slots(io: &dyn Io) -> Result<()> {
    let mut errors = Vec::new();
    for word in [0x05000000, 0x03000000, 0x04000000] {
        if let Err(error) = io.send(word) {
            errors.push(format!("{word:#x}：{error:#}"));
        }
    }
    if !errors.is_empty() {
        bail!(errors.join("；"));
    }
    Ok(())
}
fn apply(io: &dyn Io, store: &dyn Store, request: &Request) -> Result<()> {
    let plan = Plan::prepare(io, store, &request.policy)?;
    if revision(&plan.before, &plan.previous) != request.expected_revision {
        bail!("节能计划或充电策略已变化，请重新预览");
    }
    plan.execute(io, store, &mut Change::default())
}
fn restore(io: &dyn Io, store: &dyn Store, expected: &str) -> Result<()> {
    if !io.present()? {
        bail!("未确认电池存在，未恢复节能计划");
    }
    let (before, record) = observe(io, store)?.context("本机未提供节能计划能力")?;
    if revision(&before, &record) != expected {
        bail!("节能恢复预览已变化，请重新读取");
    }
    let record = record.context("没有节能初始快照")?;
    let original = record.baseline.clone();
    before.charge.same_capabilities(&original.charge)?;
    if let Some(policy) = &original.policy {
        policy.validate(&before.capability)?;
    }
    transaction::restore(io, store, before, record, original)
}
pub(super) fn charge_lock(path: &std::path::Path) -> Result<Option<&'static str>> {
    Ok(store::load(path)?.and_then(|r| {
        if r.pending.is_some() {
            Some("节能操作尚需恢复，请先恢复节能初始")
        } else if r.policy.as_ref().is_some_and(|p| p.enabled) {
            Some("节能计划管理充电阈值，请先停止节能计划")
        } else {
            None
        }
    }))
}
impl Controller {
    pub fn read_energy(&self, hal: &Hal) -> Result<Option<Profile>> {
        let _scope = self.scope()?;
        let _gate = hal.begin_write()?;
        self.with_io(hal, |_, io| {
            let mut profile = read(io, &FileStore(self.energy_path.clone()))?;
            if super::calibration::charge_lock(&self.energy_path)?.is_some() {
                if let Some(profile) = &mut profile {
                    profile.can_write = false;
                }
            }
            Ok(profile)
        })
    }
    pub fn apply_energy(&self, hal: &Hal, request: &Request) -> Result<()> {
        let scope = self.scope()?;
        let _gate = hal.begin_write()?;
        scope.ensure_available()?;
        self.with_io(hal, |_, io| {
            apply(io, &FileStore(self.energy_path.clone()), request)
        })
    }
    pub fn restore_energy(&self, hal: &Hal, expected: &str) -> Result<()> {
        let scope = self.scope()?;
        let _gate = hal.begin_write()?;
        scope.ensure_available()?;
        self.with_io(hal, |_, io| {
            restore(io, &FileStore(self.energy_path.clone()), expected)
        })
    }
}
