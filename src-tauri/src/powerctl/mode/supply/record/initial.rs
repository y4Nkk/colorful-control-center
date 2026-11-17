use super::{PlanValue, Snapshot};
use crate::gpu_power::mode as gpu;
use crate::{
    gpuctl::turbo::mode::{ClockBaseline, ClockObservation, ClockOrigin},
    powerctl::mode::workflow::StoredModeSnapshot,
};
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(in crate::powerctl::mode::supply) struct Initial {
    pub mode: StoredModeSnapshot,
    pub plans: Vec<PlanValue>,
    pub gpu_power: Option<gpu::State>,
    pub gpu_clocks: Option<ClockBaseline>,
}
impl Initial {
    pub(in crate::powerctl::mode::supply) fn new(before: Snapshot) -> Self {
        Self {
            mode: before.mode,
            plans: before.plans,
            gpu_power: before.gpu_power,
            gpu_clocks: before.gpu_clocks.raw().cloned().map(|state| ClockBaseline {
                state,
                origin: ClockOrigin::BeforeMode,
            }),
        }
    }
    pub(in crate::powerctl::mode::supply) fn validate(&self) -> Result<()> {
        self.expected().validate()?;
        if let Some(clocks) = &self.gpu_clocks {
            clocks.state.validate()?;
        }
        Ok(())
    }
    pub(in crate::powerctl::mode::supply) fn expected(&self) -> Snapshot {
        let off = self
            .gpu_power
            .as_ref()
            .is_some_and(|gpu| !gpu.target.enabled);
        Snapshot {
            mode: self.mode.clone(),
            plans: self.plans.clone(),
            gpu_power: self.gpu_power.clone(),
            gpu_clocks: match &self.gpu_clocks {
                None => ClockObservation::Untracked,
                Some(_) if off => ClockObservation::PoweredOff,
                Some(clocks) => ClockObservation::Ready(clocks.state.clone()),
            },
        }
    }
}
