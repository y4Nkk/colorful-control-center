use super::RawState;
use ccc_core::gpu_turbo::mode::SkipReason;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ClockOrigin {
    BeforeMode,
    AfterEnable,
    PoweredOff,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ClockBaseline {
    pub origin: ClockOrigin,
    pub state: RawState,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "status",
    content = "value",
    rename_all = "camelCase",
    deny_unknown_fields
)]
pub(crate) enum ClockObservation {
    Untracked,
    PoweredOff,
    Skipped(SkipReason),
    Ready(RawState),
    Unavailable(String),
}
impl ClockObservation {
    pub(crate) fn raw(&self) -> Option<&RawState> {
        match self {
            Self::Ready(value) => Some(value),
            _ => None,
        }
    }
}
