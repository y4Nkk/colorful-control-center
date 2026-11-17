use super::{Effects, clocks};
use crate::{gpu_power::mode as node, hal::Hal};
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Initial {
    #[serde(deserialize_with = "Option::deserialize")]
    pub node: Option<node::State>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub clocks: Option<clocks::ClockBaseline>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub driver_identity: Option<String>,
}
impl Initial {
    pub(crate) fn remember_node(
        hal: &Hal,
        effects: &Effects,
        original: Option<&node::State>,
    ) -> Result<()> {
        let (Some(current), Some(action)) = (&effects.node_before, effects.node_action) else {
            return Ok(());
        };
        let before = original.ok_or_else(|| anyhow::anyhow!("模式独显初始来源缺失"))?;
        before.validate()?;
        if before.id != current.id {
            bail!("模式独显初始来源属于其他设备");
        }
        if action.target == current.target && action.target == before.target {
            return Ok(());
        }
        let mut recovery = hal.recovery()?;
        let Some(initial) = recovery.oem_mode.as_mut() else {
            return Ok(());
        };
        let saved = initial.links.get_or_insert_with(Self::default);
        if saved
            .node
            .as_ref()
            .is_some_and(|value| value.id != before.id)
        {
            bail!("模式独显首次恢复记录属于其他设备");
        }
        saved.node.get_or_insert_with(|| before.clone());
        Ok(())
    }
    pub(crate) fn remember_clocks(
        hal: &Hal,
        write: &clocks::Write,
        origin: clocks::ClockOrigin,
        identity: &str,
    ) -> Result<()> {
        if write.before == write.after {
            return Ok(());
        }
        let mut recovery = hal.recovery()?;
        let initial = recovery
            .oem_mode
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("模式首次恢复快照缺失"))?;
        let saved = initial.links.get_or_insert_with(Self::default);
        if saved
            .clocks
            .as_ref()
            .is_some_and(|value| value.state.devices != write.before.devices)
        {
            bail!("模式GPU频率首次恢复记录属于其他设备");
        }
        saved.clocks.get_or_insert_with(|| clocks::ClockBaseline {
            origin,
            state: write.before.clone(),
        });
        saved
            .driver_identity
            .get_or_insert_with(|| identity.to_owned());
        Ok(())
    }
}
