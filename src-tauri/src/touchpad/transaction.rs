use super::*;

#[derive(Default)]
struct Attempts {
    state: bool,
    saved: bool,
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
    before.same_devices(&after)?;
    Ok(Plan {
        before,
        after,
        attempts: Mutex::new(Attempts::default()),
    })
}
fn verify(io: &dyn Io, expected: &Snapshot) -> Result<()> {
    if capture(io)? != *expected {
        bail!("触摸板状态在执行中变化或回读不一致");
    }
    Ok(())
}
fn write(io: &dyn Io, plan: &Plan, attempts: &mut Attempts) -> Result<()> {
    let mut expected = plan.before.clone();
    if expected.enabled != plan.after.enabled {
        attempts.state = true;
        io.set_enabled(
            &expected.devices,
            expected.available()?,
            plan.after.available()?,
        )?;
        expected.enabled = plan.after.enabled;
        verify(io, &expected)?;
    }
    if expected.saved != plan.after.saved {
        attempts.saved = true;
        io.save(plan.after.saved)?;
        expected.saved = plan.after.saved;
        verify(io, &expected)?;
    }
    Ok(())
}
pub(super) fn execute(io: &dyn Io, plan: &Plan, needs: &mut bool) -> Result<()> {
    verify(io, &plan.before)?;
    if plan.before == plan.after {
        return Ok(());
    }
    io.ensure_available()?;
    let mut attempts = plan
        .attempts
        .lock()
        .map_err(|_| anyhow!("触摸板操作记录异常"))?;
    if attempts.state || attempts.saved {
        bail!("触摸板操作仍需恢复，不能重复执行");
    }
    *needs = true;
    if let Err(error) = write(io, plan, &mut attempts) {
        let recovery = recover(io, plan, &mut attempts);
        *needs = recovery.is_err();
        bail!(ccc_core::msr::with_recovery(
            format!("触摸板设置失败：{error:#}"),
            recovery.map_err(|e| format!("{e:#}"))
        ));
    }
    Ok(())
}
fn recover_state(io: &dyn Io, plan: &Plan) -> Result<()> {
    let current = capture(io)?;
    current.same_devices(&plan.before)?;
    if current.enabled != plan.before.enabled {
        io.set_enabled(
            &current.devices,
            current.available()?,
            plan.before.available()?,
        )?;
    }
    let after = capture(io)?;
    after.same_devices(&plan.before)?;
    if after.enabled != plan.before.enabled {
        bail!("触摸板恢复回读不一致");
    }
    Ok(())
}
fn recover_saved(io: &dyn Io, plan: &Plan) -> Result<()> {
    let current = capture(io)?;
    current.same_devices(&plan.before)?;
    if current.saved == plan.before.saved {
        return Ok(());
    }
    if current.saved != plan.after.saved {
        bail!("原厂触摸板偏好已被外部修改，未覆盖");
    }
    io.save(plan.before.saved)?;
    if capture(io)?.saved != plan.before.saved {
        bail!("原厂触摸板偏好恢复回读不一致");
    }
    Ok(())
}
fn recover(io: &dyn Io, plan: &Plan, attempts: &mut Attempts) -> Result<()> {
    io.ensure_available()?;
    let mut errors = Vec::new();
    if attempts.state {
        match recover_state(io, plan) {
            Ok(()) => attempts.state = false,
            Err(e) => errors.push(format!("恢复触摸板状态：{e:#}")),
        }
    }
    if attempts.saved {
        match recover_saved(io, plan) {
            Ok(()) => attempts.saved = false,
            Err(e) => errors.push(format!("恢复原厂触摸板偏好：{e:#}")),
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
        .map_err(|_| anyhow!("触摸板操作记录异常"))?;
    recover(io, plan, &mut attempts)
}
