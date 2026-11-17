use super::{Attempt, FileStore, Record, Snapshot, record};
use crate::gpuctl::turbo::mode::{ClockBaseline, ClockObservation, ClockOrigin, Write};
use anyhow::{Result, anyhow, bail};
use ccc_core::oem_supply_mode::Trigger;

pub(super) fn checkpoint(
    store: &FileStore,
    record: &mut Record,
    write: &Write,
    origin: ClockOrigin,
    driver: &str,
) -> Result<()> {
    let attempt = record
        .attempt
        .as_ref()
        .ok_or_else(|| anyhow!("供电模式事务已经结束"))?;
    write.validate_for_mode(
        attempt
            .clock_policy
            .as_ref()
            .ok_or_else(|| anyhow!("调频策略缺失"))?,
        attempt.target.mode,
        attempt.on_ac,
        attempt.trigger == Trigger::Resume,
    )?;
    record::validate_clock_origin(origin, attempt, write)?;
    record
        .clock_approval
        .check_policy(attempt.clock_policy.as_ref().unwrap())?;
    record.clock_approval.check_driver(driver)?;
    if let Some(saved) = &attempt.clock_write {
        if saved != write
            || attempt.clock_origin != Some(origin)
            || store.load()?.as_ref() != Some(record)
        {
            bail!("GPU频率检查点或组合记录已变化");
        }
        return Ok(());
    }
    let initial = record
        .initial
        .as_ref()
        .ok_or_else(|| anyhow!("供电初始快照缺失"))?;
    if initial
        .gpu_clocks
        .as_ref()
        .is_some_and(|baseline| baseline.state.devices != write.before.devices)
    {
        bail!("GPU频率恢复基线属于其他设备");
    }
    record::update(store, record, |record| {
        record
            .clock_approval
            .driver_identity
            .get_or_insert_with(|| driver.to_owned());
        record
            .initial
            .as_mut()
            .unwrap()
            .gpu_clocks
            .get_or_insert_with(|| ClockBaseline {
                origin,
                state: write.before.clone(),
            });
        let attempt = record.attempt.as_mut().unwrap();
        attempt.clock_write = Some(write.clone());
        attempt.clock_origin = Some(origin);
    })
}

pub(super) fn confirmed(attempt: &Attempt, after: &Snapshot) -> Result<bool> {
    let decision = attempt.clock_decision()?;
    if decision.target.is_some() {
        let Some(write) = &attempt.clock_write else {
            return Ok(false);
        };
        let off = after
            .gpu_power
            .as_ref()
            .is_some_and(|gpu| !gpu.target.enabled);
        return Ok(if off {
            matches!(after.gpu_clocks, ClockObservation::PoweredOff)
        } else {
            after.gpu_clocks.raw() == Some(&write.after)
        });
    }
    Ok(attempt.clock_write.is_none() && attempt.before.gpu_clocks == after.gpu_clocks)
}
