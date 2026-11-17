use super::policy::Target;
use crate::hal::power_plan::{PowerAccess, SCHEMES};
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[cfg(test)]
mod tests;

#[derive(Clone, PartialEq, Eq)]
pub(super) struct Snapshot {
    active: u128,
    aspm: BTreeMap<u128, [u32; 2]>,
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct StoredSnapshot {
    active: String,
    aspm: BTreeMap<String, [u32; 2]>,
}
impl Snapshot {
    pub(super) fn summary(&self) -> (String, Option<[u32; 2]>) {
        (
            format!("{:032x}", self.active),
            self.aspm.get(&self.active).copied(),
        )
    }
    pub(super) fn stored(&self) -> StoredSnapshot {
        StoredSnapshot {
            active: format!("{:032x}", self.active),
            aspm: self.aspm_values(),
        }
    }
    pub(super) fn aspm_values(&self) -> BTreeMap<String, [u32; 2]> {
        self.aspm
            .iter()
            .map(|(id, values)| (format!("{id:032x}"), *values))
            .collect()
    }
    pub(super) fn merge(&mut self, before: &Self) {
        for (id, values) in &before.aspm {
            self.aspm.entry(*id).or_insert(*values);
        }
    }
    pub(super) fn plans(&self) -> Vec<RestorePlan> {
        self.aspm
            .iter()
            .map(|(id, values)| RestorePlan {
                scheme: format!("{id:032x}"),
                initial: *values,
                current: None,
            })
            .collect()
    }
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RestorePlan {
    pub scheme: String,
    pub initial: [u32; 2],
    pub current: Option<[u32; 2]>,
}
impl StoredSnapshot {
    pub(super) fn restore(&self) -> Result<Snapshot> {
        if self.active.len() != 32
            || !self
                .active
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            || self.aspm.iter().any(|(id, values)| {
                id.len() != 32
                    || !id
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                    || values.iter().any(|value| *value > 2)
            })
        {
            bail!("Windows 电源恢复记录无效");
        }
        Ok(Snapshot {
            active: u128::from_str_radix(&self.active, 16)?,
            aspm: self
                .aspm
                .iter()
                .map(|(id, values)| Ok((u128::from_str_radix(id, 16)?, *values)))
                .collect::<Result<_>>()?,
        })
    }
}

pub(super) fn capture(io: &dyn PowerAccess, target: &Target) -> Result<Option<Snapshot>> {
    if target.aspm.is_none() && target.power_plan.is_none() {
        return Ok(None);
    }
    let active = io.active()?;
    let aspm = target
        .aspm
        .map(|_| {
            io.aspm(active)
                .map(|values| BTreeMap::from([(active, values)]))
        })
        .transpose()?
        .unwrap_or_default();
    if let Some(plan) = target.power_plan {
        io.aspm(SCHEMES[usize::from(plan)])?;
    }
    Ok(Some(Snapshot { active, aspm }))
}

pub(super) fn apply(
    io: &dyn PowerAccess,
    before: &Option<Snapshot>,
    target: &Target,
    permit: &mut dyn FnMut() -> Result<()>,
) -> Result<()> {
    let Some(before) = before else {
        return Ok(());
    };
    if let Some(values) = target.aspm {
        permit()?;
        io.set_aspm(before.active, true, u32::from(values[0]))?;
        permit()?;
        io.set_aspm(before.active, false, u32::from(values[1]))?;
    }
    let plan = target
        .power_plan
        .map_or(before.active, |id| SCHEMES[usize::from(id)]);
    permit()?;
    io.activate(plan)
}

pub(super) fn restore(io: &dyn PowerAccess, before: &Option<Snapshot>) -> Result<()> {
    let Some(before) = before else {
        return Ok(());
    };
    let mut errors = Vec::new();
    for (id, values) in &before.aspm {
        for (ac, value) in [(true, values[0]), (false, values[1])] {
            if let Err(error) = io.set_aspm(*id, ac, value) {
                errors.push(format!("{error:#}"));
            }
        }
    }
    if let Err(error) = io.activate(before.active) {
        errors.push(format!("{error:#}"));
    }
    if !errors.is_empty() {
        bail!(errors.join("；"));
    }
    Ok(())
}

pub(super) fn review(
    io: &dyn PowerAccess,
    before: &Option<Snapshot>,
    target: &Target,
) -> Result<()> {
    if capture(io, target)? != *before {
        bail!("Windows电源策略或活动计划已变化，请重新预览");
    }
    Ok(())
}
