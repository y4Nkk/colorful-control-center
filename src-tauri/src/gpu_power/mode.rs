//! 模式联动的节点目标与恢复前态；供电规则和明确选择共用。
use super::{Device, Plan, Target, operation};
use anyhow::{Result, anyhow, bail};
use ccc_core::oem_supply_mode::Trigger;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Stage {
    BeforeMode,
    AfterMode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Action {
    pub stage: Stage,
    pub target: Target,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct State {
    pub id: String,
    pub name: String,
    pub target: Target,
}
impl State {
    pub(crate) fn from_device(device: &Device) -> Result<Self> {
        validate_device(device)?;
        Ok(Self {
            id: device.id.clone(),
            name: device.name.clone(),
            target: device.target(),
        })
    }
    pub(crate) fn validate(&self) -> Result<()> {
        Plan {
            id: self.id.clone(),
            expected: self.target,
            target: self.target,
        }
        .validate()?;
        if self.name.len() > 1024 || self.name.contains('\0') {
            bail!("原厂独显快照名称无效");
        }
        Ok(())
    }
}
pub(crate) fn validate_device(device: &Device) -> Result<()> {
    State {
        id: device.id.clone(),
        name: device.name.clone(),
        target: device.target(),
    }
    .validate()?;
    if !device.healthy() {
        bail!("原厂独显快照无效");
    }
    Ok(())
}

fn choice(
    mode: u8,
    trigger: Option<Trigger>,
    dc_mode_power: bool,
    automatic: bool,
) -> Result<Option<(Stage, bool)>> {
    if !ccc_core::oem_events::is_power_mode(mode) {
        bail!("未知原厂电源模式");
    }
    Ok(match (mode, trigger) {
        (2 | 3, _) => Some((Stage::BeforeMode, true)),
        (0, _) => Some((Stage::AfterMode, true)),
        (1, None) if automatic => Some((Stage::AfterMode, false)),
        (1, Some(Trigger::Resume)) if automatic && dc_mode_power => {
            Some((Stage::AfterMode, false))
        }
        _ => None,
    })
}

// None是明确的模式选择；Some携带已授权的后台供电来源。
pub(crate) fn intent(
    mode: u8,
    trigger: Option<Trigger>,
    dc_mode_power: bool,
    automatic: bool,
) -> Result<Option<Action>> {
    Ok(
        choice(mode, trigger, dc_mode_power, automatic)?.map(|(stage, enabled)| Action {
            stage,
            target: Target {
                enabled,
                automatic_power_off: automatic,
            },
        }),
    )
}

pub(crate) fn action(
    supported: bool,
    device: Option<&Device>,
    mode: u8,
    trigger: Option<Trigger>,
    dc_mode_power: bool,
) -> Result<Option<Action>> {
    if !ccc_core::oem_events::is_power_mode(mode) {
        bail!("未知原厂电源模式");
    }
    if !supported {
        if device.is_some() {
            bail!("独显节点与固件能力不一致");
        }
        return Ok(None);
    }
    let device = device.ok_or_else(|| anyhow!("原厂独显节点不可用"))?;
    validate_device(device)?;
    let Some(action) = intent(mode, trigger, dc_mode_power, device.automatic_power_off)? else {
        return Ok(None);
    };
    operation::preflight(device, device.target(), action.target)?;
    Ok(Some(action))
}

pub(crate) fn validate_action(
    action: Option<Action>,
    state: Option<&State>,
    mode: u8,
    trigger: Option<Trigger>,
) -> Result<()> {
    let Some(action) = action else { return Ok(()) };
    let state = state.ok_or_else(|| anyhow!("独显目标缺少恢复前态"))?;
    state.validate()?;
    if choice(mode, trigger, true, state.target.automatic_power_off)?
        != Some((action.stage, action.target.enabled))
        || action.target.automatic_power_off != state.target.automatic_power_off
    {
        bail!("原厂模式独显目标无效");
    }
    Ok(())
}

#[cfg(test)]
#[path = "mode/tests.rs"]
mod tests;
