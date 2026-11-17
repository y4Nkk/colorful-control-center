use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Phase {
    Discharge1,
    Charge1,
    Discharge2,
    Charge2,
    Adjusting,
}
impl Phase {
    pub fn discharge(self) -> bool {
        matches!(self, Self::Discharge1 | Self::Discharge2)
    }
    pub fn charging(self) -> bool {
        matches!(self, Self::Charge1 | Self::Charge2)
    }
    pub fn commands(self) -> [(i32, u32); 3] {
        commands(!self.discharge(), true, false)
    }
}
pub(crate) fn commands(ac: bool, refresh: bool, flexi: bool) -> [(i32, u32); 3] {
    [
        (121, 0x1c000000 | u32::from(ac)),
        (121, 0x1d000000 | u32::from(refresh)),
        (118, 0x07000000 | u32::from(flexi)),
    ]
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Status {
    Running,
    Paused,
    Completed,
    Stopped,
    Failed,
    RecoveryRequired,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Event {
    FullStatus,
    AcUnplugged,
    FullyDischarged,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Facts {
    pub present: Option<bool>,
    pub ac_online: Option<bool>,
    pub percent: Option<u8>,
    pub battery_status: u16,
    pub permanent_failure: u32,
    pub stop_threshold: u8,
}
impl Facts {
    pub fn validate(&self) -> Result<()> {
        if self.present != Some(true) {
            bail!("未确认电池存在，停止校准");
        }
        if self.ac_online.is_none() || self.percent.is_none_or(|v| v > 100) {
            bail!("电源或电量读数未知，停止校准");
        }
        if self.permanent_failure != 0 {
            bail!(
                "电池报告永久故障状态 {:#x}，停止校准",
                self.permanent_failure
            );
        }
        if !(1..=100).contains(&self.stop_threshold) {
            bail!("原厂校准停止阈值无效，未采用推测值");
        }
        Ok(())
    }
    pub fn before_start(&self) -> Result<()> {
        self.validate()?;
        if self.ac_online != Some(true) {
            bail!("请接入电源后重新预览校准");
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct State {
    pub status: Status,
    pub phase: Phase,
    pub elapsed_ms: u64,
    pub phase_elapsed_ms: u64,
    pub facts: Facts,
    pub error: Option<String>,
}
impl State {
    pub fn active(&self) -> bool {
        matches!(self.status, Status::Running | Status::Paused)
    }
    pub fn recovery_required(&self) -> bool {
        self.status == Status::RecoveryRequired
    }
}
