use crate::gpuctl::turbo::mode::{ClockObservation, Policy, Review};
use anyhow::{Result, bail};
use ccc_core::gpu_turbo::mode::SkipReason;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ClockApproval {
    pub policy_identity: String,
    #[serde(deserialize_with = "Option::deserialize")]
    pub driver_identity: Option<String>,
}
impl ClockApproval {
    pub(in crate::powerctl::mode::supply) fn new(review: &Review) -> Result<Self> {
        Ok(Self {
            policy_identity: super::super::digest(&review.policy)?,
            driver_identity: review.driver.as_ref().map(|driver| driver.identity.clone()),
        })
    }
    pub(in crate::powerctl::mode::supply) fn check_policy(&self, policy: &Policy) -> Result<()> {
        if self.policy_identity != super::super::digest(policy)? {
            bail!("GPU调频策略已变化，保留原授权和恢复基线");
        }
        Ok(())
    }
    pub(in crate::powerctl::mode::supply) fn check_review(&self, review: &Review) -> Result<()> {
        self.check_policy(&review.policy)?;
        if let Some(driver) = &review.driver {
            self.check_driver(&driver.identity)?;
        }
        Ok(())
    }
    pub(in crate::powerctl::mode::supply) fn check_driver(&self, identity: &str) -> Result<()> {
        if !valid_identity(identity)
            || self
                .driver_identity
                .as_ref()
                .is_some_and(|id| id != identity)
        {
            bail!("GPU设备或驱动能力已变化，不能沿用调频授权");
        }
        Ok(())
    }
    pub(super) fn validate(&self) -> Result<()> {
        if !valid_identity(&self.policy_identity)
            || self
                .driver_identity
                .as_ref()
                .is_some_and(|id| !valid_identity(id))
        {
            bail!("GPU调频授权记录无效");
        }
        Ok(())
    }
}
fn valid_identity(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[derive(Clone, Copy)]
pub(in crate::powerctl::mode::supply) enum ClockRead {
    Untracked,
    Skip(SkipReason),
    Tracked,
    Inspect,
}
impl ClockRead {
    pub(in crate::powerctl::mode::supply) fn for_observation(
        value: &ClockObservation,
    ) -> Result<Self> {
        Ok(match value {
            ClockObservation::Untracked => Self::Untracked,
            ClockObservation::Skipped(reason) => Self::Skip(*reason),
            ClockObservation::PoweredOff | ClockObservation::Ready(_) => Self::Tracked,
            ClockObservation::Unavailable(error) => bail!("GPU频率前态不可用：{error}"),
        })
    }
    pub(in crate::powerctl::mode::supply) fn preview(tracked: bool) -> Self {
        if tracked {
            Self::Inspect
        } else {
            Self::Untracked
        }
    }
    pub(in crate::powerctl::mode::supply) fn capture(
        self,
        io: &impl super::super::transaction::Io,
    ) -> Result<ClockObservation> {
        match self {
            Self::Untracked => Ok(ClockObservation::Untracked),
            Self::Skip(reason) => Ok(ClockObservation::Skipped(reason)),
            Self::Tracked => Ok(match io.clocks()? {
                Some(value) => ClockObservation::Ready(value),
                None => ClockObservation::PoweredOff,
            }),
            Self::Inspect => Ok(match io.clocks() {
                Ok(Some(value)) => ClockObservation::Ready(value),
                Ok(None) => ClockObservation::PoweredOff,
                Err(error) => ClockObservation::Unavailable(format!("{error:#}")),
            }),
        }
    }
}
