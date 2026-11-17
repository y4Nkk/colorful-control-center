use super::{Context, Effects, clocks};
use crate::gpu_power::{Device, mode as node};
use anyhow::{Context as _, Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Target {
    pub node_supported: bool,
    #[serde(deserialize_with = "Option::deserialize")]
    pub node: Option<Device>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub action: Option<node::Action>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub on_ac: Option<bool>,
    pub clocks: clocks::Selection,
    #[serde(deserialize_with = "Option::deserialize")]
    pub stages: Option<crate::gpu_power::profile::Stages>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "status",
    content = "value",
    rename_all = "camelCase",
    deny_unknown_fields
)]
pub(crate) enum Review {
    Ready(Target),
    Unavailable(String),
}
impl Review {
    pub(crate) fn unavailable_reason(&self) -> Option<&str> {
        match self {
            Self::Unavailable(reason) => Some(reason),
            Self::Ready(target) => target.unavailable_reason(),
        }
    }
    pub(crate) fn target(&self) -> Result<&Target> {
        if let Some(reason) = self.unavailable_reason() {
            bail!("模式GPU联动不可执行：{reason}");
        }
        match self {
            Self::Ready(target) => Ok(target),
            Self::Unavailable(_) => unreachable!(),
        }
    }
}
impl Target {
    pub(crate) fn unavailable_reason(&self) -> Option<&str> {
        if self.on_ac.is_none() {
            return Some("实际供电未知，请重读模式GPU目标");
        }
        self.clocks.unavailable_reason()
    }
    pub(crate) fn effects(&self) -> Result<Effects> {
        if let Some(reason) = self.unavailable_reason() {
            bail!("模式GPU联动不可执行：{reason}");
        }
        let decision = self
            .clocks
            .candidate
            .decision
            .as_ref()
            .context("GPU模式目标缺失")?;
        let before = if let Some(reason) = decision.skipped {
            clocks::ClockObservation::Skipped(reason)
        } else if self.node.as_ref().is_some_and(|device| !device.enabled) {
            clocks::ClockObservation::PoweredOff
        } else {
            clocks::ClockObservation::Ready(
                self.clocks
                    .driver
                    .as_ref()
                    .context("GPU驱动前态缺失")?
                    .current
                    .clone(),
            )
        };
        Ok(Effects {
            node_before: self
                .node
                .as_ref()
                .map(node::State::from_device)
                .transpose()?,
            node_action: self.action,
            clock_policy: Some(self.clocks.policy.clone()),
            clock_before: before,
            on_ac: self.on_ac.context("实际供电未知")?,
            trigger: None,
        })
    }
}
impl Context<'_> {
    pub(crate) fn selection(&self, mode: u8) -> Review {
        match self.selection_target(mode) {
            Ok(target) => Review::Ready(target),
            Err(error) => Review::Unavailable(format!("{error:#}")),
        }
    }
    fn selection_target(&self, mode: u8) -> Result<Target> {
        let supported = self.power.supported_in_control(self.hal)?;
        let device = self.device()?;
        let action = node::action(supported, device.as_ref(), mode, None, false)?;
        let on_ac = self.source.on_ac()?;
        let mut clocks = clocks::selection(self.policy()?, mode, on_ac);
        self.clocks.inspect_mode_selection_in_control(
            &mut clocks,
            self.source.as_ref(),
            mode,
            on_ac,
            device.as_ref().is_some_and(|device| !device.enabled),
        );
        Ok(Target {
            node_supported: supported,
            node: device,
            action,
            on_ac,
            clocks,
            stages: None,
        })
    }
}
