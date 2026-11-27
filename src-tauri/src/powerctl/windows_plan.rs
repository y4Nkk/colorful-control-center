use crate::hal::{
    Hal,
    power_plan::{format_guid, parse_guid},
};
use anyhow::{Result, bail, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[cfg(test)]
mod tests;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Profile {
    revision: String,
    active_id: SchemeId,
    plans: Vec<Plan>,
    blocked_reason: Option<String>,
}

#[derive(Serialize)]
struct Plan {
    id: SchemeId,
    name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Request {
    expected_revision: String,
    target_id: SchemeId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
struct SchemeId(u128);

impl TryFrom<String> for SchemeId {
    type Error = anyhow::Error;

    fn try_from(value: String) -> Result<Self> {
        Ok(Self(parse_guid(&value)?))
    }
}

impl From<SchemeId> for String {
    fn from(id: SchemeId) -> Self {
        format_guid(id.0)
    }
}

fn capture(hal: &Hal) -> Result<Profile> {
    let io = hal.system_power.as_ref();
    let mut plans = io
        .schemes()?
        .into_iter()
        .map(|scheme| {
            Ok(Plan {
                id: SchemeId(scheme),
                name: io.scheme_name(scheme)?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    plans.sort_unstable_by_key(|plan| plan.id);
    let active_id = SchemeId(io.active()?);
    ensure!(
        plans.iter().any(|plan| plan.id == active_id),
        "当前计划不在 Windows 返回的列表中，请重新读取"
    );
    let blocked_reason = hal
        .ensure_mode_change_allowed()
        .and_then(|()| io.check_active_scheme_write())
        .err()
        .map(|error| format!("{error:#}"));
    let revision = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&(active_id, &plans, &blocked_reason))?)
    );
    Ok(Profile {
        revision,
        active_id,
        plans,
        blocked_reason,
    })
}

pub(crate) fn read(hal: &Hal) -> Result<Profile> {
    let _gate = hal.begin_write()?;
    capture(hal)
}

pub(crate) fn apply(hal: &Hal, request: &Request) -> Result<()> {
    let _gate = hal.begin_write()?;
    let current = capture(hal)?;
    if let Some(reason) = current.blocked_reason {
        bail!("{reason}");
    }
    ensure!(
        current.revision == request.expected_revision,
        "电源计划已变化，请重新选择"
    );
    ensure!(
        current
            .plans
            .iter()
            .any(|plan| plan.id == request.target_id),
        "目标电源计划不存在"
    );
    if request.target_id == current.active_id {
        return Ok(());
    }

    let io = hal.system_power.as_ref();
    ensure!(
        SchemeId(io.active()?) == current.active_id,
        "当前电源计划已变化，请重新选择"
    );
    io.activate(request.target_id.0)
}
