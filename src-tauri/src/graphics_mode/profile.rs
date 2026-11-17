use super::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Target {
    pub owner: String,
    pub available_mask: u8,
    pub mode: u8,
}
impl Target {
    pub fn validate(&self) -> Result<()> {
        if self.owner.len() != 64
            || !self
                .owner
                .bytes()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        {
            bail!("档案显卡模式缺少有效的Windows安装标识");
        }
        Observed {
            mode: self.mode,
            available_mask: self.available_mask,
        }
        .validate()?;
        if self.available_mask & !15 != 0 {
            bail!("档案显卡模式候选包含未知标志");
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Review {
    pub before: Profile,
    pub target: u8,
    pub software_fan_running: bool,
}
pub(super) fn known(p: &Preview) -> u8 {
    p.loaded
        .record
        .as_ref()
        .filter(|record| record.boot == p.identity.boot && record.phase == Phase::Accepted)
        .map_or(p.observed.mode, |record| record.target)
}
pub(super) fn capture(p: &Preview) -> Result<Target> {
    if p.loaded.record.as_ref().is_some_and(|record| {
        record.verification(&p.observed, &p.identity) == Verification::Uncertain
    }) {
        bail!("显卡选择中断，不能将未知挂起选择捕获为档案");
    }
    Ok(Target {
        owner: p.identity.owner.clone(),
        available_mask: p.observed.available_mask,
        mode: known(p),
    })
}
impl Review {
    pub fn changes(&self) -> bool {
        let known = self
            .before
            .last_request
            .as_ref()
            .filter(|request| request.verification == Verification::PendingReboot)
            .map(|request| request.target)
            .or_else(|| self.before.current.as_ref().map(|current| current.mode));
        known != Some(self.target)
    }
}
