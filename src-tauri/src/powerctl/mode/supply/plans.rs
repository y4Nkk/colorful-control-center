use super::Review;
use crate::{
    hal::power_plan::{PowerAccess, SCHEMES},
    powerctl::mode::workflow::StoredModeSnapshot,
};
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct PlanValue {
    pub scheme: String,
    pub aspm: [u32; 2],
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlanChange {
    pub scheme: String,
    pub current: [u32; 2],
    pub initial: [u32; 2],
}

fn scheme(value: &str) -> Result<u128> {
    if value.len() != 32
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        bail!("电源计划标识无效");
    }
    Ok(u128::from_str_radix(value, 16)?)
}

pub(super) fn validate(values: &[PlanValue]) -> Result<()> {
    if values.len() > 4 {
        bail!("供电模式电源计划数量无效");
    }
    let mut seen = BTreeSet::new();
    for value in values {
        if !seen.insert(scheme(&value.scheme)?) || value.aspm.iter().any(|v| *v > 2) {
            bail!("供电模式电源计划恢复值无效");
        }
    }
    Ok(())
}

pub(super) fn validate_initial(snapshot: &StoredModeSnapshot, values: &[PlanValue]) -> Result<()> {
    validate(values)?;
    let owned = snapshot.system_plan_values()?;
    if owned.is_empty() && !values.is_empty() {
        bail!("没有节能快照，不能恢复电源计划节能值");
    }
    if owned.iter().any(|(id, before)| {
        values
            .iter()
            .find(|value| &value.scheme == id)
            .map(|value| value.aspm)
            != Some(*before)
    }) || values.iter().any(|value| {
        !owned.contains_key(&value.scheme)
            && !SCHEMES
                .iter()
                .any(|id| format!("{id:032x}") == value.scheme)
    }) {
        bail!("电源计划恢复范围与初始快照不一致");
    }
    Ok(())
}

pub(super) fn initial(
    io: &dyn PowerAccess,
    review: &Review,
    snapshot: &StoredModeSnapshot,
) -> Result<Vec<PlanValue>> {
    let owned = snapshot.system_plan_values()?;
    if owned.is_empty() {
        if review.targets.iter().any(|target| target.aspm.is_some()) {
            bail!("电源计划初始节能快照缺失");
        }
        return Ok(Vec::new());
    }
    let mut schemes = owned
        .keys()
        .map(|id| scheme(id))
        .collect::<Result<BTreeSet<_>>>()?;
    for index in review.targets.iter().filter_map(|target| target.power_plan) {
        schemes.insert(
            *SCHEMES
                .get(usize::from(index))
                .ok_or_else(|| anyhow::anyhow!("电源计划目标无效"))?,
        );
    }
    let values = schemes
        .into_iter()
        .map(|id| {
            Ok(PlanValue {
                scheme: format!("{id:032x}"),
                aspm: io.aspm(id)?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    validate_initial(snapshot, &values)?;
    Ok(values)
}

pub(super) fn current(io: &dyn PowerAccess, initial: &[PlanValue]) -> Result<Vec<PlanValue>> {
    validate(initial)?;
    initial
        .iter()
        .map(|value| {
            Ok(PlanValue {
                scheme: value.scheme.clone(),
                aspm: io.aspm(scheme(&value.scheme)?)?,
            })
        })
        .collect()
}

pub(super) fn changes(initial: &[PlanValue], current: &[PlanValue]) -> Result<Vec<PlanChange>> {
    validate(initial)?;
    validate(current)?;
    if initial.len() != current.len()
        || initial
            .iter()
            .zip(current)
            .any(|(a, b)| a.scheme != b.scheme)
    {
        bail!("供电模式电源计划集合已变化");
    }
    Ok(initial
        .iter()
        .zip(current)
        .map(|(before, now)| PlanChange {
            scheme: before.scheme.clone(),
            current: now.aspm,
            initial: before.aspm,
        })
        .collect())
}

pub(super) fn restore_other_plans(
    io: &dyn PowerAccess,
    initial: &[PlanValue],
    expected: &[PlanValue],
    initial_snapshot: &StoredModeSnapshot,
    permit: &mut dyn FnMut() -> Result<()>,
) -> Result<()> {
    changes(initial, expected)?;
    let owned = initial_snapshot.system_plan_values()?;
    for (before, now) in initial.iter().zip(expected) {
        if owned.contains_key(&before.scheme) || before.aspm == now.aspm {
            continue;
        }
        let id = scheme(&before.scheme)?;
        if io.aspm(id)? != now.aspm {
            bail!("恢复预览后的电源计划节能值已变化");
        }
        for (index, on_ac) in [true, false].into_iter().enumerate() {
            if before.aspm[index] != now.aspm[index] {
                permit()?;
                let mut wanted = now.aspm;
                wanted[..index].copy_from_slice(&before.aspm[..index]);
                if io.aspm(id)? != wanted {
                    bail!("恢复期间电源计划节能值已变化");
                }
                io.set_aspm(id, on_ac, before.aspm[index])?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
