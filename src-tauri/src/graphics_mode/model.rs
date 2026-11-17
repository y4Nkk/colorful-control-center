use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Observed {
    pub mode: u8,
    pub available_mask: u8,
}
impl Observed {
    pub fn supports(&self, mode: u8) -> bool {
        (1..=4).contains(&mode) && self.available_mask & (1 << (mode - 1)) != 0
    }
    pub fn validate(&self) -> Result<()> {
        if !self.supports(self.mode) {
            bail!("未知的原厂当前显卡模式或能力列表");
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Identity {
    pub owner: String,
    pub boot: String,
}
fn hex(text: &str, len: usize) -> bool {
    text.len() == len
        && text
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}
impl Identity {
    pub fn validate(&self) -> Result<()> {
        if !hex(&self.owner, 64) || !hex(&self.boot, 32) || self.boot.bytes().all(|c| c == b'0') {
            bail!("无法确认Windows安装或系统启动标识");
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub enum Phase {
    Prepared,
    Accepted,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Record {
    pub version: u8,
    pub owner: String,
    pub boot: String,
    pub initial_mode: u8,
    pub previous_mode: u8,
    pub target: u8,
    pub available_mask: u8,
    pub phase: Phase,
}
impl Record {
    pub fn validate(&self) -> Result<()> {
        if self.version != 1 {
            bail!("不支持的显卡选择记录版本");
        }
        Identity {
            owner: self.owner.clone(),
            boot: self.boot.clone(),
        }
        .validate()?;
        let observed = Observed {
            mode: self.initial_mode,
            available_mask: self.available_mask,
        };
        if ![self.initial_mode, self.previous_mode, self.target]
            .into_iter()
            .all(|mode| observed.supports(mode))
        {
            bail!("显卡选择记录含无效模式");
        }
        Ok(())
    }
    pub fn previous_known(&self) -> u8 {
        if self.phase == Phase::Accepted {
            self.target
        } else {
            self.previous_mode
        }
    }
    pub fn verification(&self, observed: &Observed, identity: &Identity) -> Verification {
        if self.boot != identity.boot && observed.mode == self.target {
            Verification::Confirmed
        } else if self.phase == Phase::Prepared {
            Verification::Uncertain
        } else if self.boot == identity.boot {
            Verification::PendingReboot
        } else {
            Verification::NotApplied
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Verification {
    Uncertain,
    PendingReboot,
    Confirmed,
    NotApplied,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LastRequest {
    pub initial_mode: u8,
    pub target: u8,
    pub verification: Verification,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Profile {
    pub current: Option<Observed>,
    pub revision: String,
    pub last_request: Option<LastRequest>,
    pub can_restore: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub expected_revision: String,
    pub target: u8,
}
