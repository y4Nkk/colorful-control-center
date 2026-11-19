use super::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PlanId {
    Balanced,
    HighPerformance,
    PowerSaver,
}
pub const PLANS: [PlanId; 3] = [
    PlanId::Balanced,
    PlanId::HighPerformance,
    PlanId::PowerSaver,
];
impl PlanId {
    pub fn guid(self) -> u128 {
        SCHEMES[match self {
            Self::Balanced => 0,
            Self::HighPerformance => 1,
            Self::PowerSaver => 2,
        }]
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Targets {
    pub ac: Option<bool>,
    pub dc: Option<bool>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub expected_revision: String,
    pub values: Targets,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlanState {
    pub id: PlanId,
    pub ac: Option<bool>,
    pub dc: Option<bool>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Target {
    pub plans: Vec<PlanState>,
}
impl Target {
    pub fn validate(&self) -> Result<()> {
        let ids: BTreeSet<_> = self.plans.iter().map(|plan| plan.id).collect();
        if self.plans.is_empty() || ids.len() != self.plans.len() {
            bail!("USB档案计划为空或重复");
        }
        if !self
            .plans
            .iter()
            .any(|plan| plan.ac.is_some() || plan.dc.is_some())
        {
            bail!("USB档案没有已知目标字段");
        }
        Ok(())
    }
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub revision: String,
    pub recommendation: Option<bool>,
    pub supported: bool,
    pub active_plan: Option<PlanId>,
    pub plans: Vec<PlanState>,
    pub missing: Vec<PlanId>,
    pub initial: Vec<PlanState>,
    pub blocked_reason: Option<String>,
    pub restore_blocked_reason: Option<String>,
    pub warnings: Vec<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(super) struct Frame {
    pub firmware: Option<u8>,
    pub active: u128,
    pub plans: BTreeMap<PlanId, [u32; 2]>,
}
pub(super) type Baseline = BTreeMap<PlanId, [Option<u32>; 2]>;
#[derive(Clone, Copy)]
pub(crate) struct Change {
    pub id: PlanId,
    pub ac: bool,
    pub before: u32,
    pub target: u32,
}
impl Frame {
    pub fn revision(&self) -> Result<String> {
        Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(self)?)))
    }
    pub fn review(&self, expected: &str) -> Result<()> {
        if self.revision()? != expected {
            bail!("USB 选择性暂停、系统计划或固件建议已变化，请重新读取后重试");
        }
        Ok(())
    }
    pub fn changes(&self, target: &Targets) -> Result<Vec<Change>> {
        if target.ac.is_none() && target.dc.is_none() {
            bail!("请选择插电或电池供电的USB目标");
        }
        self.changes_for(&Target {
            plans: self
                .plans
                .keys()
                .map(|id| PlanState {
                    id: *id,
                    ac: target.ac,
                    dc: target.dc,
                })
                .collect(),
        })
    }
    pub fn profile_target(&self) -> Target {
        Target {
            plans: self
                .plans
                .iter()
                .map(|(id, values)| PlanState {
                    id: *id,
                    ac: flag(values[0]),
                    dc: flag(values[1]),
                })
                .collect(),
        }
    }
    pub fn changes_for(&self, target: &Target) -> Result<Vec<Change>> {
        target.validate()?;
        if self.firmware.is_none() {
            bail!("固件未声明USB 选择性暂停控制，只能恢复本次修改");
        }
        let mut changes = Vec::new();
        for plan in &target.plans {
            let current = self
                .plans
                .get(&plan.id)
                .context("USB档案对应的标准计划不存在")?;
            for (index, desired) in [plan.ac, plan.dc].into_iter().enumerate() {
                if let Some(desired) = desired {
                    if current[index] > 1 {
                        bail!("USB原始设置未知，无法保证该字段恢复");
                    }
                    if current[index] != u32::from(desired) {
                        changes.push(Change {
                            id: plan.id,
                            ac: index == 0,
                            before: current[index],
                            target: u32::from(desired),
                        });
                    }
                }
            }
        }
        Ok(changes)
    }
    pub fn initial_changes(&self, baseline: &Baseline) -> Result<Vec<Change>> {
        if baseline.is_empty() {
            bail!("没有本次USB修改的初始快照");
        }
        let mut changes = Vec::new();
        for (id, fields) in baseline {
            let current = self
                .plans
                .get(id)
                .context("初始快照对应的电源计划已被删除，未写入恢复")?;
            for (index, target) in fields.iter().enumerate() {
                if let Some(target) = target {
                    if current[index] != *target {
                        changes.push(Change {
                            id: *id,
                            ac: index == 0,
                            before: current[index],
                            target: *target,
                        });
                    }
                }
            }
        }
        Ok(changes)
    }
}
pub fn flag(raw: u32) -> Option<bool> {
    match raw {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    }
}
pub fn supported(bytes: &[u8]) -> Result<bool> {
    if bytes.len() != 24 {
        bail!("USB扩展能力页长度无效");
    }
    match &bytes[..2] {
        [0, 0] => Ok(false),
        [1, 0] => Ok(bytes[23] & 1 != 0),
        _ => bail!("未知USB能力页版本"),
    }
}
