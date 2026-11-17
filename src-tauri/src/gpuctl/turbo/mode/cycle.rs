use super::{Candidate, DriverReview, Policy, candidate, driver};
use crate::gpuctl::{
    GpuController,
    turbo::{ensure_turbo, source::SourceIo},
};
use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Choice {
    pub mode: u8,
    pub on_ac: bool,
    pub candidate: Candidate,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Cycle {
    pub policy: Policy,
    pub choices: Vec<Choice>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub driver: Option<DriverReview>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub driver_error: Option<String>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub deferred_reason: Option<String>,
}
impl Cycle {
    pub(crate) fn new(policy: Policy, modes: &[u8]) -> Self {
        let choices = modes
            .iter()
            .flat_map(|&mode| {
                [true, false].map(|on_ac| Choice {
                    mode,
                    on_ac,
                    candidate: candidate(&policy, Some(mode), Some(on_ac), false),
                })
            })
            .collect();
        Self {
            policy,
            choices,
            driver: None,
            driver_error: None,
            deferred_reason: None,
        }
    }
    pub(crate) fn unavailable_reason(&self) -> Option<&str> {
        self.driver_error.as_deref().or_else(|| {
            self.choices
                .iter()
                .find_map(|choice| choice.candidate.unavailable_reason.as_deref())
        })
    }
}
impl GpuController {
    pub(crate) fn inspect_mode_cycle_in_control(
        &self,
        review: &mut Cycle,
        source: &dyn SourceIo,
        on_ac: Option<bool>,
        powered_off: bool,
    ) {
        if !review.choices.iter().any(|choice| {
            choice
                .candidate
                .decision
                .as_ref()
                .is_some_and(|decision| decision.target.is_some())
        }) {
            return;
        }
        if powered_off {
            review.deferred_reason = Some(driver::DEFERRED_REASON.into());
            return;
        }
        let inspected: Result<()> = (|| {
            let on_ac = on_ac.ok_or_else(|| anyhow!("实际供电未知，不能检查GPU驱动目标"))?;
            let mut state = self.state.lock().map_err(|_| anyhow!("GPU控制状态异常"))?;
            let (snapshot, driver) = driver::capture(
                ensure_turbo(&mut state)?.as_ref(),
                source,
                &review.policy,
                on_ac,
            )?;
            for choice in &mut review.choices {
                driver::inspect_candidate(
                    &snapshot,
                    &driver.current,
                    &mut choice.candidate,
                    choice.mode,
                    choice.on_ac,
                );
            }
            review.driver = Some(driver);
            Ok(())
        })();
        if let Err(error) = inspected {
            review.driver_error = Some(format!("{error:#}"));
        }
    }
}
