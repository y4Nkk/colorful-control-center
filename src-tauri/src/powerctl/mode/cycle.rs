use super::{links, selection};
use crate::{
    gpu_power::{Device, mode as node},
    gpuctl::turbo::mode as clocks,
};
use anyhow::{Context, Result, bail};
use serde::Serialize;
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NodeChoice {
    pub mode: u8,
    pub action: Option<node::Action>,
    pub unavailable_reason: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Profile {
    pub modes: Vec<super::Target>,
    pub node_supported: bool,
    pub node: Option<Device>,
    pub node_choices: Vec<NodeChoice>,
    pub clocks: clocks::Cycle,
}
impl Profile {
    pub(crate) fn identity(&self) -> Result<String> {
        if let Some(reason) = self.clocks.unavailable_reason().or_else(|| {
            self.node_choices
                .iter()
                .find_map(|choice| choice.unavailable_reason.as_deref())
        }) {
            bail!("{reason}");
        }
        let modes: Vec<_> = self
            .modes
            .iter()
            .map(|mode| (mode.mode, mode.ac_mode, mode.energy))
            .collect();
        let node = self
            .node
            .as_ref()
            .map(|device| (&device.id, &device.name, device.automatic_power_off));
        let decisions: Vec<_> = self
            .clocks
            .choices
            .iter()
            .map(|choice| (choice.mode, choice.on_ac, &choice.candidate.decision))
            .collect();
        Ok(format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&(
                "oem-mode-cycle-gpu-1",
                modes,
                self.node_supported,
                node,
                &self.node_choices,
                &self.clocks.policy,
                decisions,
            ))?)
        ))
    }
    pub(crate) fn driver_identity(&self) -> Option<&str> {
        self.clocks
            .driver
            .as_ref()
            .map(|driver| driver.identity.as_str())
    }
    pub(crate) fn check_driver(&self, expected: Option<&str>) -> Result<()> {
        if expected
            .zip(self.driver_identity())
            .is_some_and(|(expected, actual)| expected != actual)
        {
            bail!("模式循环的GPU设备或驱动能力已变化，请重新预览确认");
        }
        Ok(())
    }
}
pub(crate) fn read(context: &links::Context<'_>) -> Result<Profile> {
    let modes = super::read(context.hal)?;
    let first = *modes
        .available_modes
        .first()
        .context("固件未开放电源模式")?;
    let gpu = match context.selection(first) {
        links::Review::Ready(target) => target,
        links::Review::Unavailable(reason) => bail!("{reason}"),
    };
    let node_choices = modes
        .available_modes
        .iter()
        .map(|&mode| {
            let action = node::action(gpu.node_supported, gpu.node.as_ref(), mode, None, false);
            NodeChoice {
                mode,
                unavailable_reason: action.as_ref().err().map(|error| format!("{error:#}")),
                action: action.ok().flatten(),
            }
        })
        .collect();
    let mut clocks = clocks::Cycle::new(gpu.clocks.policy, &modes.available_modes);
    context.inspect_cycle(
        &mut clocks,
        gpu.on_ac,
        gpu.node.as_ref().is_some_and(|device| !device.enabled),
    );
    Ok(Profile {
        modes: modes.targets,
        node_supported: gpu.node_supported,
        node: gpu.node,
        node_choices,
        clocks,
    })
}
pub(crate) fn execute(
    context: links::Context<'_>,
    expected: &str,
    driver: Option<&str>,
    checkpoint: &mut dyn FnMut(&str) -> Result<()>,
    permit: &mut dyn FnMut() -> Result<()>,
) -> Result<u8> {
    let target = {
        let _gate = context.hal.begin_write()?;
        permit()?;
        let profile = read(&context)?;
        if profile.identity()? != expected {
            bail!("模式循环策略已变化，请重新预览确认");
        }
        profile.check_driver(driver)?;
        let current = super::read(context.hal)?;
        let wanted =
            ccc_core::oem_events::next_mode(current.current_mode, &current.available_modes)
                .map_err(anyhow::Error::msg)?;
        selection::checkpoint(&context, wanted, None)?.target
    };
    selection::apply_checkpoint(
        context,
        &target,
        &mut |_, _, identity| {
            if driver.is_some_and(|expected| expected != identity) {
                bail!("模式循环的GPU驱动能力已变化，未执行调频");
            }
            checkpoint(identity)
        },
        permit,
    )?;
    Ok(target.mode)
}
