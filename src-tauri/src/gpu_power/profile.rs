//! 档案独显目标与只读确认事实；节点和自动关断始终分别保存。
use super::*;
use crate::hal::Hal;
use serde::{Deserialize, Serialize};
#[path = "profile/stages.rs"]
mod stages;
pub(crate) use stages::Stages;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Target {
    pub id: String,
    pub name: String,
    pub values: model::Target,
}
impl Target {
    pub fn validate(&self) -> Result<()> {
        model::Plan {
            id: self.id.clone(),
            expected: self.values,
            target: self.values,
        }
        .validate()?;
        if self.name.len() > 1024 || self.name.contains('\0') {
            bail!("档案独显名称无效");
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Review {
    pub before: Device,
    pub target: model::Target,
    pub software_fan_running: bool,
}
impl Review {
    pub(crate) fn independent_stages(&self) -> Result<Stages> {
        Stages::build(
            &self.before,
            None,
            Some(&Target {
                id: self.before.id.clone(),
                name: self.before.name.clone(),
                values: self.target,
            }),
            self.needs_gpu(),
        )
    }
    pub fn needs_gpu(&self) -> bool {
        !self.before.enabled && self.target.enabled
    }
}
impl scope::Scope<'_> {
    pub(crate) fn capture(&self, hal: &Hal) -> Result<Option<Target>> {
        let devices = self.list(hal)?;
        let device = match devices.as_slice() {
            [] => return Ok(None),
            [device] if device.healthy() => device,
            _ => bail!("独显控制节点不唯一或存在故障，无法捕获档案"),
        };
        let target = Target {
            id: device.id.clone(),
            name: device.name.clone(),
            values: device.target(),
        };
        target.validate()?;
        Ok(Some(target))
    }
    pub(crate) fn preview_profile(&self, hal: &Hal, target: &Target) -> Result<Review> {
        target.validate()?;
        let current = self
            .list(hal)?
            .into_iter()
            .find(|d| d.id == target.id)
            .ok_or_else(|| anyhow!("档案独显控制实例不存在"))?;
        operation::preflight(&current, current.target(), target.values)?;
        Ok(Review {
            before: current,
            target: target.values,
            software_fan_running: crate::fanctl::software::read_for_handoff(hal)?.is_some(),
        })
    }
}
