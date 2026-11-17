use super::*;
use model::{Baseline, Change, Frame, Request};

fn active_matches(power: &dyn PowerAccess, expected: u128) -> Result<()> {
    if power.active()? != expected {
        bail!("活动电源计划已变化，未切回旧计划");
    }
    Ok(())
}
fn set(power: &dyn PowerAccess, change: &Change, value: u32) -> Result<()> {
    power.set_usb(change.id.guid(), change.ac, value)?;
    if power.usb(change.id.guid())?[usize::from(!change.ac)] != value {
        bail!("USB策略写后回读不一致");
    }
    Ok(())
}
fn refresh_current(power: &dyn PowerAccess) -> Result<()> {
    let current = power.active()?;
    active_matches(power, current)?;
    power.activate(current)
}
pub(super) fn compensate(power: &dyn PowerAccess, attempted: &[Change]) -> Result<()> {
    let mut errors = Vec::new();
    for change in attempted.iter().rev() {
        if let Err(error) = set(power, change, change.before) {
            errors.push(format!("{:?}恢复：{error:#}", change.id));
        }
    }
    if let Err(error) = refresh_current(power) {
        errors.push(format!("重载当前计划：{error:#}"));
    }
    if !errors.is_empty() {
        bail!(errors.join("；"));
    }
    Ok(())
}
pub(super) fn apply(
    io: &(impl Io + ?Sized),
    baseline: &mut Baseline,
    before: Frame,
    request: &Request,
) -> Result<()> {
    before.review(&request.expected_revision)?;
    let changes = before.changes(&request.values)?;
    execute(io, baseline, &before, &changes, &mut Vec::new())
}
pub(super) fn execute(
    io: &(impl Io + ?Sized),
    baseline: &mut Baseline,
    before: &Frame,
    changes: &[Change],
    attempted: &mut Vec<Change>,
) -> Result<()> {
    io.ensure_write()?;
    if changes.is_empty() {
        return Ok(());
    }
    let power = io.power();
    let result = (|| -> Result<()> {
        for change in changes {
            active_matches(power, before.active)?;
            if power.usb(change.id.guid())?[usize::from(!change.ac)] != change.before {
                bail!("目标USB字段被外部修改");
            }
            baseline.entry(change.id).or_insert([None; 2])[usize::from(!change.ac)]
                .get_or_insert(change.before);
            attempted.push(*change);
            set(power, &change, change.target)?;
        }
        active_matches(power, before.active)?;
        power.activate(before.active)
    })();
    if let Err(error) = result {
        let recovery = if attempted.is_empty() {
            Ok(())
        } else {
            compensate(power, attempted)
        };
        if recovery.is_ok() {
            attempted.clear();
        }
        bail!(
            "{}",
            ccc_core::msr::with_recovery(
                format!("USB策略应用失败：{error:#}"),
                recovery.map_err(|e| format!("{e:#}"))
            )
        );
    }
    Ok(())
}
pub(super) fn restore(
    io: &(impl Io + ?Sized),
    baseline: &Baseline,
    before: Frame,
    expected: &str,
) -> Result<()> {
    before.review(expected)?;
    let changes = before.initial_changes(baseline)?;
    io.ensure_write()?;
    if changes.is_empty() {
        return Ok(());
    }
    let power = io.power();
    active_matches(power, before.active)?;
    let mut errors = Vec::new();
    for change in changes {
        if let Err(error) = set(power, &change, change.target) {
            errors.push(format!("{:?}初始恢复：{error:#}", change.id));
        }
    }
    if let Err(error) = refresh_current(power) {
        errors.push(format!("重载当前计划：{error:#}"));
    }
    if !errors.is_empty() {
        bail!(errors.join("；"));
    }
    Ok(())
}
