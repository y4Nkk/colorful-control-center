use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Rule {
    pub version: [u8; 2],
    pub customer: u8,
    pub feature: bool,
}
impl Rule {
    pub fn validate(&self) -> Result<()> {
        if !matches!(self.version, [0, 0] | [1, 0]) {
            bail!("低电量亮度能力版本未知");
        }
        Ok(())
    }
    pub fn low_supported(&self) -> bool {
        self.customer == 5 || self.version == [1, 0] && self.feature
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Snapshot {
    pub rule: Rule,
    pub panel: String,
    pub plan: String,
    pub levels: [u32; 2],
    pub cache: [u8; 2],
}
impl Snapshot {
    pub fn scheme(&self) -> Result<u128> {
        if self.plan.len() != 32
            || !self
                .plan
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            bail!("低电量亮度计划标识无效");
        }
        Ok(u128::from_str_radix(&self.plan, 16)?)
    }
    pub fn validate(&self) -> Result<()> {
        self.rule.validate()?;
        self.scheme()?;
        if self.panel.is_empty() || self.panel.len() > 1024 {
            bail!("低电量亮度面板标识无效");
        }
        if self.levels.iter().any(|value| *value > 100)
            || self.cache.iter().any(|value| *value > 100)
        {
            bail!("低电量亮度或原厂缓存超出百分比范围");
        }
        Ok(())
    }
    pub fn setting(&self, percent: u8) -> Result<Self> {
        self.validate()?;
        if percent > 100 {
            bail!("电池电量超出百分比范围");
        }
        let mut target = self.clone();
        if self.levels[0] > 0 && self.cache[0] != 0 {
            target.cache[0] = self.levels[0] as u8;
        } else if self.cache[0] == 0 {
            target.cache[0] = self.levels[0] as u8;
            target.levels = [0; 2];
        }
        target.cache[1] = percent;
        Ok(target)
    }
    pub fn ending(&self) -> Result<Self> {
        self.validate()?;
        let mut target = self.clone();
        if self.cache[0] != 0 {
            let value = if self.levels[0] == 0 {
                self.cache[0] as u32
            } else {
                self.levels[0]
            };
            target.levels = [value; 2];
            target.cache[0] = 0;
        }
        Ok(target)
    }
    pub fn identity(&self) -> Result<String> {
        self.validate()?;
        super::super::super::model::digest(&(
            "battery-low-brightness-1",
            &self.rule,
            &self.panel,
            &self.plan,
        ))
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
pub struct Attempt {
    pub before: Snapshot,
    pub target: Snapshot,
    pub levels: [bool; 2],
    pub cache: [bool; 2],
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Record {
    pub version: u8,
    pub phase: Phase,
    pub baseline: Snapshot,
    pub current: Snapshot,
    pub levels_owned: [bool; 2],
    pub cache_owned: [bool; 2],
    pub pending: Option<Attempt>,
}
impl Record {
    pub fn new(snapshot: Snapshot) -> Result<Self> {
        snapshot.validate()?;
        Ok(Self {
            version: 2,
            phase: Phase::Applied,
            current: snapshot.clone(),
            baseline: snapshot,
            levels_owned: [false; 2],
            cache_owned: [false; 2],
            pending: None,
        })
    }
    pub fn validate(&self) -> Result<()> {
        self.baseline.validate()?;
        self.current.validate()?;
        if self.version != 2
            || self.baseline.identity()? != self.current.identity()?
            || self.phase == Phase::Preparing && self.pending.is_none()
            || self.phase == Phase::Applied && self.pending.is_some()
        {
            bail!("低电量亮度恢复记录无效");
        }
        if let Some(attempt) = &self.pending {
            attempt.before.validate()?;
            attempt.target.validate()?;
            if attempt.before.identity()? != self.baseline.identity()?
                || attempt.target.identity()? != self.baseline.identity()?
                || attempt.before.cache != self.current.cache
            {
                bail!("低电量亮度尝试来源无效");
            }
            for index in 0..2 {
                if attempt.levels[index]
                    && (!self.levels_owned[index]
                        || attempt.before.levels[index] == attempt.target.levels[index])
                    || attempt.cache[index]
                        && (!self.cache_owned[index]
                            || attempt.before.cache[index] == attempt.target.cache[index])
                {
                    bail!("低电量亮度尝试字段无效");
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Recovery {
    pub record: Record,
    pub active: String,
    pub panel: String,
    pub levels: [u32; 2],
    pub cache: [u8; 2],
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub identity: Option<String>,
    pub current: Option<Snapshot>,
    pub setting_target: Option<[u32; 2]>,
    pub ending_target: Option<[u32; 2]>,
    pub on_ac: Option<bool>,
    pub percent: Option<u8>,
    pub error: Option<String>,
    pub recovery: Option<Recovery>,
    pub restore_revision: Option<String>,
    pub restore_error: Option<String>,
}
