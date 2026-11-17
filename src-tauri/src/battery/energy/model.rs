use super::super::model::{ChargeState, ChargeValues};
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Limits {
    pub floor: u8,
    pub ceiling: u8,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Slot {
    pub start: u8,
    pub end: u8,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Policy {
    pub enabled: bool,
    pub every_day: bool,
    pub weekdays: u8,
    pub slots: [Option<Slot>; 2],
    pub automatic: bool,
    pub limits: Limits,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Capability {
    pub factory: Limits,
    pub factory_defaulted: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Target {
    pub capability: Capability,
    pub policy: Policy,
}
impl Target {
    pub fn validate(&self) -> Result<()> {
        if self.capability.factory.floor >= self.capability.factory.ceiling
            || self.capability.factory.ceiling > 100
        {
            bail!("节能档案的原厂阈值无效");
        }
        self.policy.validate(&self.capability)
    }
}
impl Policy {
    pub fn validate(&self, capability: &Capability) -> Result<()> {
        if self.weekdays > 127 || self.enabled && !self.every_day && self.weekdays == 0 {
            bail!("请选择有效的节能星期");
        }
        for slot in self.slots.iter().flatten() {
            if slot.start > 23 || slot.end > 24 || slot.start >= slot.end {
                bail!("节能时段须在0–24点内，开始早于结束");
            }
        }
        if let [Some(a), Some(b)] = &self.slots {
            if a.start < b.end && b.start < a.end {
                bail!("两个节能时段不能重叠");
            }
        }
        let l = &self.limits;
        if self.automatic {
            if *l != capability.factory {
                bail!("原厂自动阈值已变化，请重新读取");
            }
        } else if ![40, 50, 60, 70, 80, 90].contains(&l.floor)
            || ![60, 70, 80, 90, 100].contains(&l.ceiling)
        {
            bail!("请选择原厂提供的节能阈值档位");
        }
        if l.floor >= l.ceiling || l.ceiling > 100 {
            bail!("放电下限须低于充电上限");
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Snapshot {
    pub policy: Option<Policy>,
    pub charge: ChargeState,
    pub capability: Capability,
}
impl Snapshot {
    pub fn validate(&self) -> Result<()> {
        self.charge.validate_snapshot()?;
        if let Some(policy) = &self.policy {
            policy.validate(&self.capability)?;
        }
        if self.capability.factory.floor >= self.capability.factory.ceiling
            || self.capability.factory.ceiling > 100
        {
            bail!("节能快照的原厂阈值无效");
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Record {
    pub version: u8,
    pub policy: Option<Policy>,
    pub capability: Capability,
    pub baseline: Snapshot,
    pub stop_charge: ChargeState,
    pub pending: Option<Snapshot>,
    pub last_error: Option<String>,
}
impl Record {
    pub fn validate(&self) -> Result<()> {
        if self.version != 1 {
            bail!("不支持的节能记录版本");
        }
        if let Some(policy) = &self.policy {
            policy.validate(&self.capability)?;
        }
        self.baseline.validate()?;
        self.stop_charge.validate_snapshot()?;
        if self.capability != self.baseline.capability {
            bail!("节能记录能力不一致");
        }
        if let Some(pending) = &self.pending {
            pending.validate()?;
            if pending.capability != self.capability {
                bail!("节能恢复能力不一致");
            }
        }
        Ok(())
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub expected_revision: String,
    pub policy: Policy,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub saved: Option<Policy>,
    pub capability: Capability,
    pub revision: String,
    pub source: &'static str,
    pub can_restore: bool,
    pub recovery_required: bool,
    pub charge_on_stop: ChargeState,
    pub charge_reported: ChargeValues,
    pub has_initial_plan: bool,
    pub initial_charge: ChargeValues,
    pub error: Option<String>,
    pub can_write: bool,
}
#[derive(Clone, Copy)]
pub struct Clock {
    pub weekday: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}
