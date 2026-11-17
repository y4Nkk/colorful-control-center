//! 电源模式预览使用的GPU资格与原厂目标，不初始化任何保存字段。
use crate::hal::{Hal, oem_identity::Model};
use anyhow::{Result, anyhow};
pub(crate) use ccc_core::gpu_turbo::mode::{Decision, Policy};
use ccc_core::gpu_turbo::{Capabilities, Limits, Offsets};
use serde::{Deserialize, Serialize};
#[path = "mode/raw.rs"]
mod raw;
pub(crate) use raw::RawState;
#[path = "mode/baseline.rs"]
mod baseline;
pub(crate) use baseline::{ClockBaseline, ClockObservation, ClockOrigin};
#[path = "mode/execution.rs"]
mod execution;
pub(crate) use execution::{Prepared, Write};
#[path = "mode/driver.rs"]
mod driver;
#[path = "mode/session.rs"]
mod session;
pub(crate) use driver::DriverReview;
#[path = "mode/selection.rs"]
mod selection;
pub(crate) use selection::{Selection, selection};
#[path = "mode/cycle.rs"]
mod cycle;
pub(crate) use cycle::Cycle;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Candidate {
    #[serde(deserialize_with = "Option::deserialize")]
    pub decision: Option<Decision>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub unavailable_reason: Option<String>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub projected: Option<RawState>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Review {
    pub policy: Policy,
    pub ac: Candidate,
    pub battery: Candidate,
    pub startup_ac: Candidate,
    pub startup_battery: Candidate,
    pub resume_ac: Candidate,
    pub resume_battery: Candidate,
    pub driver: Option<DriverReview>,
    pub driver_error: Option<String>,
    pub deferred_reason: Option<String>,
}

impl Review {
    #[cfg(test)]
    pub(crate) fn available(&self) -> bool {
        self.unavailable_reason().is_none()
    }
    pub(crate) fn candidates(&self) -> [&Candidate; 6] {
        [
            &self.ac,
            &self.battery,
            &self.startup_ac,
            &self.startup_battery,
            &self.resume_ac,
            &self.resume_battery,
        ]
    }
    pub(crate) fn unavailable_reason(&self) -> Option<&str> {
        self.driver_error.as_deref().or_else(|| {
            self.candidates()
                .into_iter()
                .find_map(|candidate| candidate.unavailable_reason.as_deref())
        })
    }
}

pub(super) fn candidate(
    policy: &Policy,
    mode: Option<u8>,
    source: Option<bool>,
    resume: bool,
) -> Candidate {
    match mode
        .map(|mode| policy.decide(mode, source, resume))
        .transpose()
    {
        Ok(decision) => Candidate {
            decision,
            unavailable_reason: None,
            projected: None,
        },
        Err(error) => Candidate {
            decision: None,
            unavailable_reason: Some(error),
            projected: None,
        },
    }
}
pub(crate) fn review(policy: Policy, modes: [Option<u8>; 6]) -> Review {
    let make = |mode, source, resume| candidate(&policy, mode, source, resume);
    Review {
        ac: make(modes[0], Some(true), false),
        battery: make(modes[1], Some(false), false),
        startup_ac: make(modes[2], Some(true), false),
        startup_battery: make(modes[3], Some(false), false),
        resume_ac: make(modes[4], Some(true), true),
        resume_battery: make(modes[5], Some(false), true),
        policy,
        driver: None,
        driver_error: None,
        deferred_reason: None,
    }
}

pub(crate) fn read(hal: &Hal, model: &Model) -> Result<Policy> {
    hal.with_dchu(|io| {
        let mut request = [0; 256];
        let packet = io.bios_capabilities()?;
        let legacy = Capabilities::needs_wmi96(&packet).map_err(anyhow::Error::msg)?;
        let legacy_flags = |command| {
            let (valid, value) = io.read_int(command);
            valid.then_some(value)
        };
        let wmi16 = legacy.then(|| legacy_flags(16)).flatten();
        let wmi96 = legacy.then(|| legacy_flags(96)).flatten();
        request[0] = 9;
        let limits = Limits::from_packet(&io.exchange_package(4, &request)?);
        let cache: [u8; 13] = io
            .read_settings(5, 0, 13)?
            .try_into()
            .map_err(|_| anyhow!("原厂GPU报告长度无效"))?;
        let saved = Offsets::saved(&io.read_settings(5, 16, 8)?).map_err(anyhow::Error::msg)?;
        Policy::from_packet(
            &packet,
            wmi16,
            wmi96,
            model.product,
            model.customer,
            &cache,
            limits,
            saved,
        )
        .map_err(anyhow::Error::msg)
    })
}

#[cfg(test)]
pub(crate) fn test_policy() -> Policy {
    Policy {
        capabilities: Some(Capabilities {
            limit_clock: false,
            all_mode_oc: false,
        }),
        limits: Limits {
            entertainment_mhz: 0,
            performance_mhz: 0,
            saving_mhz: 0,
            quiet_mhz: 0,
            thermal_mhz: 0,
        },
        saved: [Offsets {
            core_mhz: 0,
            memory_mhz: 0,
        }; 2],
        gpu_oc: Some(false),
        simple_gpu_oc: false,
        ready: false,
        gpu_count: 0,
        cached_device_id: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ccc_core::gpu_turbo::mode::SkipReason;

    #[test]
    fn supply_preview_uses_each_modes_source_and_the_distinct_resume_rule() {
        let mut policy = test_policy();
        policy.ready = true;
        policy.gpu_count = 2;
        policy.gpu_oc = Some(true);
        policy.capabilities.as_mut().unwrap().limit_clock = true;
        policy.limits.performance_mhz = 1700;
        policy.saved = [Offsets {
            core_mhz: 100,
            memory_mhz: -1,
        }; 2];
        let view = review(
            policy,
            [Some(2), Some(1), Some(3), Some(1), Some(2), Some(1)],
        );
        assert!(view.available());
        let ac = view.ac.decision.as_ref().unwrap().target.as_ref().unwrap();
        assert_eq!(ac.lock_mhz, Some(1700));
        assert_eq!(ac.offsets.as_ref().unwrap()[0].core_mhz, 100);
        let dc = view
            .battery
            .decision
            .as_ref()
            .unwrap()
            .target
            .as_ref()
            .unwrap();
        assert_eq!(dc.offsets.as_ref().unwrap()[0].core_mhz, 0);
        assert!(view.startup_ac.decision.as_ref().unwrap().target.is_some());
        assert!(
            view.startup_battery
                .decision
                .as_ref()
                .unwrap()
                .target
                .is_some()
        );
        assert_eq!(
            view.resume_ac.decision.as_ref().unwrap().skipped,
            Some(SkipReason::Resume)
        );
    }

    #[test]
    fn unknown_capability_is_reported_only_for_a_mode_that_might_write() {
        let mut policy = test_policy();
        policy.ready = true;
        policy.gpu_oc = None;
        let view = review(policy.clone(), [Some(2); 6]);
        assert!(!view.available());
        assert!(
            view.ac
                .unavailable_reason
                .as_ref()
                .unwrap()
                .contains("WMI 16")
        );
        assert_eq!(
            view.resume_battery.decision.unwrap().skipped,
            Some(SkipReason::Resume)
        );
        let quiet_plus = review(policy, [Some(4); 6]);
        assert!(quiet_plus.available());
        assert_eq!(
            quiet_plus.ac.decision.unwrap().skipped,
            Some(SkipReason::QuietPlus)
        );
    }
}

#[cfg(test)]
#[path = "mode/source_tests.rs"]
mod source_tests;
