use super::*;
use ccc_core::settings::{self, Patch};

fn save(io: &impl Io, raw: &[u8; 5]) -> Result<()> {
    settings::apply(io, 4, &[Patch::new(81, *raw)], None).map_err(|e| anyhow!(e))?;
    if field::<5>(io, 4, 81)? != *raw {
        bail!("除尘计划保存回读不一致");
    }
    Ok(())
}
pub(super) fn recover(io: &impl Io, raw: &[u8; 5]) -> Result<()> {
    let schedule = protocol::decode(raw)?;
    let clock = if schedule.is_some_and(|s| s.enabled()) {
        require_idle(io)?;
        io.sync_clock().context("恢复除尘时钟")
    } else {
        Ok(())
    };
    require_idle(io)?;
    let mut errors = Vec::new();
    for (name, result) in [
        (
            "恢复固件计划",
            clock.and_then(|_| io.command(40, schedule.map_or(0, |s| s.command()))),
        ),
        ("恢复保存字段", save(io, raw)),
    ] {
        if let Err(error) = result {
            errors.push(format!("{name}：{error:#}"));
        }
    }
    if !errors.is_empty() {
        bail!(errors.join("；"));
    }
    Ok(())
}
pub(super) fn apply(io: &impl Io, state: &mut State, before: &Facts, raw: [u8; 5]) -> Result<()> {
    apply_tracked(io, state, before, raw, &mut false)
}
pub(super) fn apply_tracked(
    io: &impl Io,
    state: &mut State,
    before: &Facts,
    raw: [u8; 5],
    needs_rollback: &mut bool,
) -> Result<()> {
    if before.saved == raw {
        return Ok(());
    }
    let schedule = protocol::decode(&raw)?;
    if schedule.is_some_and(|s| s.enabled()) {
        io.sync_clock()?;
    }
    require_idle(io)?;
    state.baseline.get_or_insert(before.saved);
    *needs_rollback = true;
    let result: Result<()> = (|| {
        io.command(40, schedule.map_or(0, |s| s.command()))?;
        save(io, &raw)?;
        io.running()?.context("除尘能力在写入期间变化")?;
        Ok(())
    })();
    if let Err(error) = result {
        let restored = recover(io, &before.saved).map_err(|e| format!("{e:#}"));
        if restored.is_ok() {
            *needs_rollback = false;
        }
        bail!(ccc_core::msr::with_recovery(format!("{error:#}"), restored));
    }
    Ok(())
}
pub(super) fn restore(io: &impl Io, state: &State) -> Result<()> {
    let raw = state.baseline.as_ref().context("没有除尘计划初始快照")?;
    recover(io, raw)
}
