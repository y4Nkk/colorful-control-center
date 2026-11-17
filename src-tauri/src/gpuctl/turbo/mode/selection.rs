use super::{Candidate, DriverReview, Policy, candidate, driver};
use crate::gpuctl::{
    GpuController,
    turbo::{ensure_turbo, source::SourceIo},
};
use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Selection {
    pub policy: Policy,
    pub candidate: Candidate,
    #[serde(deserialize_with = "Option::deserialize")]
    pub driver: Option<DriverReview>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub driver_error: Option<String>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub deferred_reason: Option<String>,
}
impl Selection {
    pub(crate) fn unavailable_reason(&self) -> Option<&str> {
        self.driver_error
            .as_deref()
            .or(self.candidate.unavailable_reason.as_deref())
    }
}
pub(crate) fn selection(policy: Policy, mode: u8, on_ac: Option<bool>) -> Selection {
    let candidate = candidate(&policy, Some(mode), on_ac, false);
    Selection {
        policy,
        candidate,
        driver: None,
        driver_error: None,
        deferred_reason: None,
    }
}
impl GpuController {
    pub(crate) fn inspect_mode_selection_in_control(
        &self,
        review: &mut Selection,
        source: &dyn SourceIo,
        mode: u8,
        on_ac: Option<bool>,
        powered_off: bool,
    ) {
        if !review
            .candidate
            .decision
            .as_ref()
            .is_some_and(|decision| decision.target.is_some())
        {
            return;
        }
        if powered_off {
            review.deferred_reason = Some(driver::DEFERRED_REASON.into());
            return;
        }
        let result: Result<()> = (|| {
            let on_ac = on_ac.ok_or_else(|| anyhow!("实际供电未知，不能检查GPU驱动目标"))?;
            let mut state = self.state.lock().map_err(|_| anyhow!("GPU控制状态异常"))?;
            let (snapshot, driver) = driver::capture(
                ensure_turbo(&mut state)?.as_ref(),
                source,
                &review.policy,
                on_ac,
            )?;
            driver::inspect_candidate(
                &snapshot,
                &driver.current,
                &mut review.candidate,
                mode,
                on_ac,
            );
            review.driver = Some(driver);
            Ok(())
        })();
        if let Err(error) = result {
            review.driver_error = Some(format!("{error:#}"));
        }
    }
}
