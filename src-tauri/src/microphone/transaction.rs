use super::*;

#[derive(Default)]
struct Attempts {
    endpoints: BTreeMap<String, bool>,
    saved: Option<u8>,
}
pub(crate) struct Plan {
    pub(super) before: Snapshot,
    after: Snapshot,
    attempts: Mutex<Attempts>,
}
pub(super) fn capture(io: &dyn Io) -> Result<Snapshot> {
    let snapshot = io.snapshot()?;
    snapshot.validate()?;
    Ok(snapshot)
}
pub(super) fn prepare(before: Snapshot, after: Snapshot) -> Result<Plan> {
    before.validate()?;
    after.validate()?;
    before.same_devices(&after)?;
    Ok(Plan {
        before,
        after,
        attempts: Mutex::new(Attempts::default()),
    })
}
fn verify(io: &dyn Io, expected: &Snapshot) -> Result<()> {
    if capture(io)? != *expected {
        bail!("麦克风状态在执行中变化或回读不一致");
    }
    Ok(())
}
fn write(io: &dyn Io, plan: &Plan, attempts: &mut Attempts) -> Result<()> {
    let mut expected = plan.before.clone();
    for (index, target) in plan.after.endpoints.iter().enumerate() {
        if target.muted == expected.endpoints[index].muted {
            continue;
        }
        attempts
            .endpoints
            .insert(target.id.clone(), expected.endpoints[index].muted);
        io.set_muted(&target.id, target.muted)?;
        expected.endpoints[index].muted = target.muted;
        verify(io, &expected)?;
    }
    if expected.saved != plan.after.saved {
        attempts.saved = Some(expected.saved);
        io.save(plan.after.saved)?;
        expected.saved = plan.after.saved;
        verify(io, &expected)?;
    }
    Ok(())
}
pub(super) fn execute(
    io: &dyn Io,
    plan: &Plan,
    needs_rollback: &mut bool,
) -> Result<()> {
    verify(io, &plan.before)?;
    if plan.before == plan.after {
        return Ok(());
    }
    io.ensure_available()?;
    let mut attempts = plan
        .attempts
        .lock()
        .map_err(|_| anyhow!("麦克风操作记录异常"))?;
    if !attempts.endpoints.is_empty() || attempts.saved.is_some() {
        bail!("麦克风操作仍需恢复，不能重复执行");
    }
    *needs_rollback = true;
    if let Err(error) = write(io, plan, &mut attempts) {
        let recovery = recover(io, plan, &mut attempts);
        *needs_rollback = recovery.is_err();
        bail!(ccc_core::msr::with_recovery(
            format!("麦克风设置失败：{error:#}"),
            recovery.map_err(|e| format!("{e:#}"))
        ));
    }
    Ok(())
}
fn recover_endpoint(io: &dyn Io, id: &str, muted: bool) -> Result<()> {
    let current = capture(io)?;
    let value = current
        .endpoints
        .iter()
        .find(|endpoint| endpoint.id == id)
        .context("原麦克风已断开")?;
    if value.muted != muted {
        io.set_muted(id, muted)?;
    }
    let after = capture(io)?;
    if after
        .endpoints
        .iter()
        .find(|endpoint| endpoint.id == id)
        .map(|e| e.muted)
        != Some(muted)
    {
        bail!("麦克风恢复回读不一致");
    }
    Ok(())
}
fn recover_saved(io: &dyn Io, plan: &Plan, value: u8) -> Result<()> {
    let current = capture(io)?.saved;
    if current == value {
        return Ok(());
    }
    if current != plan.after.saved {
        bail!("原厂静音偏好已被外部修改，未覆盖");
    }
    io.save(value)?;
    if capture(io)?.saved != value {
        bail!("原厂静音偏好恢复回读不一致");
    }
    Ok(())
}
fn recover(io: &dyn Io, plan: &Plan, attempts: &mut Attempts) -> Result<()> {
    io.ensure_available()?;
    let mut errors = Vec::new();
    for (id, value) in attempts.endpoints.clone() {
        match recover_endpoint(io, &id, value) {
            Ok(()) => {
                attempts.endpoints.remove(&id);
            }
            Err(error) => errors.push(format!("恢复麦克风{id}：{error:#}")),
        }
    }
    if let Some(value) = attempts.saved {
        match recover_saved(io, plan, value) {
            Ok(()) => attempts.saved = None,
            Err(error) => errors.push(format!("恢复原厂静音偏好：{error:#}")),
        }
    }
    if !errors.is_empty() {
        bail!(errors.join("；"));
    }
    Ok(())
}
pub(super) fn rollback(io: &dyn Io, plan: &Plan) -> Result<()> {
    let mut attempts = plan
        .attempts
        .lock()
        .map_err(|_| anyhow!("麦克风操作记录异常"))?;
    recover(io, plan, &mut attempts)
}
