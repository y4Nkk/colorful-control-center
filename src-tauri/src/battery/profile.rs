use super::*;
use energy::{FileStore, Store};

pub(crate) struct Plan {
    energy: Option<energy::Plan>,
    charge: Option<charge::Plan>,
}
#[derive(Default)]
pub(crate) struct Change {
    energy: energy::Change,
    charge: bool,
}
impl Change {
    pub fn dirty(&self) -> bool {
        self.charge || self.energy.dirty()
    }
}
pub(crate) fn validate_targets(
    charge: Option<&ChargeTarget>,
    energy: Option<&energy::Target>,
) -> Result<()> {
    if let Some(target) = charge {
        target.validate()?;
    }
    if let Some(target) = energy {
        target.validate()?;
        if target.policy.enabled && charge.is_some() {
            bail!("启用节能计划与独立充电阈值不能同时配置");
        }
    }
    Ok(())
}
impl Scope<'_> {
    pub fn capture(&self, hal: &Hal) -> Result<(Option<ChargeTarget>, Option<energy::Target>)> {
        self.ensure_available()?;
        self.controller.with_io(hal, |io, energy| {
            let store = FileStore(self.controller.energy_path.clone());
            if store.load()?.is_some_and(|record| record.pending.is_some()) {
                bail!("节能操作尚需恢复，不能捕获电池档案");
            }
            let observed = energy::observe(energy, &store)?;
            let charge = io.charge()?;
            if observed.is_none() && charge.is_none() {
                return Ok((None, None));
            }
            if io.power()?.present != Some(true) {
                bail!("未确认电池存在，不能捕获电池档案");
            }
            let target = observed.and_then(|(snapshot, _)| {
                snapshot.policy.map(|policy| energy::Target {
                    policy,
                    capability: snapshot.capability,
                })
            });
            if let Some(target) = &target {
                target.validate()?;
            }
            let charge = if target.as_ref().is_some_and(|target| target.policy.enabled) {
                None
            } else {
                charge.map(ChargeTarget::capture)
            };
            if let Some(target) = &charge {
                target.validate()?;
            }
            Ok((charge, target))
        })
    }
    pub fn prepare(
        &self,
        hal: &Hal,
        charge: Option<&ChargeTarget>,
        energy: Option<&energy::Target>,
    ) -> Result<Plan> {
        self.ensure_available()?;
        validate_targets(charge, energy)?;
        self.controller.with_io(hal, |io, energy_io| {
            let store = FileStore(self.controller.energy_path.clone());
            let plan = energy
                .map(|target| {
                    target.validate()?;
                    let plan = energy::Plan::prepare(energy_io, &store, &target.policy)?;
                    if plan.before.capability != target.capability {
                        bail!("节能档案能力与当前设备不一致");
                    }
                    Ok(plan)
                })
                .transpose()?;
            let charge = charge
                .map(|target| {
                    target.validate()?;
                    if io.power()?.present != Some(true) {
                        bail!("未确认电池存在，未修改充电保护");
                    }
                    let current = current(io)?;
                    current.same_capabilities(&target.state())?;
                    let projected = if let Some(plan) = &plan {
                        if current != plan.before.charge {
                            bail!("充电与节能快照已变化，请重新预览");
                        }
                        plan.charge.clone()
                    } else {
                        self.ensure_charge_available(energy_io)?;
                        current
                    };
                    charge::Plan::profile(projected, target)
                })
                .transpose()?;
            Ok(Plan {
                energy: plan,
                charge,
            })
        })
    }
    pub fn execute(&mut self, hal: &Hal, plan: &Plan, change: &mut Change) -> Result<()> {
        self.ensure_available()?;
        self.controller.with_io(hal, |io, energy| {
            let store = FileStore(self.controller.energy_path.clone());
            if let Some(plan) = &plan.energy {
                plan.execute(energy, &store, &mut change.energy)?;
            }
            if let Some(plan) = &plan.charge {
                self.ensure_charge_available(energy)?;
                plan.execute(io, &mut change.charge)?;
            }
            Ok(())
        })
    }
    pub fn rollback(&self, hal: &Hal, plan: &Plan, change: &mut Change) -> Result<()> {
        self.ensure_available()?;
        self.controller.with_io(hal, |io, energy| {
            let mut errors = Vec::new();
            if let Some(plan) = plan.charge.as_ref().filter(|_| change.charge) {
                let restored = self
                    .ensure_charge_available(energy)
                    .and_then(|_| plan.rollback(io));
                if let Err(error) = restored {
                    errors.push(format!("充电保护：{error:#}"));
                } else {
                    change.charge = false;
                }
            }
            if let Some(plan) = &plan.energy {
                if let Err(error) = plan.rollback(
                    energy,
                    &FileStore(self.controller.energy_path.clone()),
                    &mut change.energy,
                ) {
                    errors.push(format!("节能计划：{error:#}"));
                }
            }
            if !errors.is_empty() {
                bail!(errors.join("；"));
            }
            Ok(())
        })
    }
}
