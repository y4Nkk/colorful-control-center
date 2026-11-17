use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const TARGETS: [u32; 3] = [25, 20, 22];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Plan {
    pub id: String,
    pub values: [[u32; 2]; 3],
}
impl Plan {
    pub fn scheme(&self) -> Result<u128> {
        if self.id.len() != 32
            || !self
                .id
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            bail!("电池计划标识无效");
        }
        Ok(u128::from_str_radix(&self.id, 16)?)
    }
    fn validate(&self) -> Result<()> {
        self.scheme()?;
        if self.values.iter().flatten().any(|v| *v > 100) {
            bail!("电池计划阈值超出百分比范围");
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Snapshot {
    pub active: String,
    pub plans: Vec<Plan>,
    pub cache: [u8; 3],
}
impl Snapshot {
    pub fn validate(&self) -> Result<()> {
        if self.plans.is_empty() || self.plans.len() > 4 || self.plans[0].id != self.active {
            bail!("电池计划集合无效");
        }
        let mut seen = BTreeSet::new();
        for plan in &self.plans {
            plan.validate()?;
            if !seen.insert(&plan.id) {
                bail!("电池计划集合重复");
            }
        }
        let available = crate::hal::power_plan::SCHEMES.map(|id| format!("{id:032x}"));
        let expected: Vec<_> = available
            .iter()
            .filter(|id| *id != &self.active && seen.contains(*id))
            .collect();
        if self
            .plans
            .iter()
            .skip(1)
            .map(|plan| &plan.id)
            .collect::<Vec<_>>()
            != expected
        {
            bail!("电池计划恢复范围不符合活动与标准计划");
        }
        if self.cache.iter().any(|v| *v > 100) {
            bail!("原厂电池阈值缓存无效");
        }
        Ok(())
    }
    pub fn identity(&self) -> Result<String> {
        self.validate()?;
        super::super::model::digest(&("battery-threshold-events-1", self))
    }
    pub fn cache_target(&self) -> [u8; 3] {
        std::array::from_fn(|i| self.plans[0].values[i][1] as u8)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Phase {
    Preparing,
    Applied,
    Restoring,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Record {
    pub version: u8,
    pub phase: Phase,
    pub baseline: Snapshot,
    pub owned: Vec<[[bool; 2]; 3]>,
    pub cache_owned: [bool; 3],
}
impl Record {
    pub fn new(snapshot: Snapshot) -> Result<Self> {
        snapshot.validate()?;
        if snapshot.cache != [0; 3] {
            bail!("原厂电池阈值缓存已存在且没有本应用记录，不能推测其余计划原值");
        }
        Ok(Self {
            version: 1,
            phase: Phase::Preparing,
            owned: vec![[[false; 2]; 3]; snapshot.plans.len()],
            baseline: snapshot,
            cache_owned: [false; 3],
        })
    }
    pub fn validate(&self) -> Result<()> {
        self.baseline.validate()?;
        if self.version != 1
            || self.owned.len() != self.baseline.plans.len()
            || self.baseline.cache != [0; 3]
        {
            bail!("电池阈值恢复记录无效");
        }
        for (index, plan) in self.baseline.plans.iter().enumerate() {
            for (level, target) in TARGETS.into_iter().enumerate() {
                for supply in 0..2 {
                    let differs = plan.values[level][supply] != target;
                    if self.owned[index][level][supply] && !differs
                        || self.phase == Phase::Applied
                            && self.owned[index][level][supply] != differs
                    {
                        bail!("电池阈值恢复字段与基线不一致");
                    }
                }
            }
        }
        for (index, target) in self.baseline.cache_target().into_iter().enumerate() {
            let differs = self.baseline.cache[index] != target;
            if self.cache_owned[index] && !differs
                || self.phase == Phase::Applied && self.cache_owned[index] != differs
            {
                bail!("电池阈值缓存恢复字段与基线不一致");
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Recovery {
    pub phase: Phase,
    pub baseline: Snapshot,
    pub current_active: String,
    pub current: Vec<Plan>,
    pub cache: [u8; 3],
    pub owned: Vec<[[bool; 2]; 3]>,
    pub cache_owned: [bool; 3],
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub identity: Option<String>,
    pub current: Option<Snapshot>,
    pub error: Option<String>,
    pub recovery: Option<Recovery>,
    pub restore_revision: Option<String>,
    pub restore_error: Option<String>,
}
