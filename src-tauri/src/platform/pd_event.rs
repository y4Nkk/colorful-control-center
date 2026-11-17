use super::*;
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub identity: Option<String>,
    pub current: Option<PowerDelivery>,
    pub error: Option<String>,
}
fn target_identity(target: &Target) -> Result<String> {
    target.validate()?;
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&("oem-pd-event-46", target))?)
    ))
}
fn from_view(current: &PowerDelivery) -> Result<String> {
    if let Some(reason) = current
        .blocked_reason
        .as_ref()
        .or(current.restore_blocked_reason.as_ref())
    {
        bail!(reason.clone());
    }
    if !current.current_valid {
        bail!("当前PD等级不在候选中，不能保证恢复");
    }
    target_identity(&Target {
        level: 1,
        encoding: current.encoding,
        adapter_watts: current.adapter_watts,
        available_levels: current.available_levels.clone(),
    })
}
pub(super) fn preview(
    io: &impl Io,
    baseline: &Option<Snapshot>,
    blocked: Option<String>,
) -> Result<Profile> {
    let profile = read(io, baseline, blocked)?;
    let current = profile.power_delivery;
    let result = current
        .as_ref()
        .ok_or_else(|| {
            anyhow!(
                "{}",
                profile
                    .warnings
                    .first()
                    .map(String::as_str)
                    .unwrap_or("固件未声明PD档位控制")
            )
        })
        .and_then(from_view);
    Ok(Profile {
        identity: result.as_ref().ok().cloned(),
        error: result.err().map(|error| format!("{error:#}")),
        current,
    })
}
pub(super) fn execute(
    io: &impl Io,
    baseline: &mut Option<Snapshot>,
    expected: &str,
    permit: &mut impl FnMut() -> Result<()>,
) -> Result<bool> {
    permit()?;
    let before = capture(io)?;
    before.validate_current()?;
    if let Some(initial) = baseline.as_ref() {
        before.restorable(initial)?;
    }
    let mut target = before.profile_target();
    target.level = 1;
    if target_identity(&target)? != expected {
        bail!("PD事件的适配器、编号或候选已变化，请重新确认");
    }
    let planned = before.clone();
    let plan = Plan::prepare(before, baseline, 1)?;
    let mut touched = false;
    plan.execute_permitted(io, baseline, &mut touched, &mut || {
        permit()?;
        if capture(io)? != planned {
            bail!("许可复核期间PD档位或适配器已变化");
        }
        Ok(())
    })?;
    Ok(touched)
}
