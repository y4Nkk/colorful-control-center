use super::{
    model::*,
    store::FileStore,
    transaction::{self, Io, LEVELS},
};
use anyhow::{Result, bail};

fn guard(
    io: &impl Io,
    store: &FileStore,
    record: &Record,
    active: u128,
    permit: &mut impl FnMut() -> Result<()>,
) -> Result<()> {
    permit()?;
    io.ensure_writable()?;
    if store.load()?.as_ref() != Some(record) {
        bail!("电池阈值恢复记录已变化，停止输出");
    }
    if io.active()? != active {
        bail!("活动电源计划变化，停止恢复并保留记录");
    }
    let recovery = transaction::recovery(io, record)?;
    if recovery.current_active != format!("{active:032x}") {
        bail!("活动电源计划变化，停止恢复并保留记录");
    }
    transaction::compatible(&recovery)
}
fn restore_plan(
    io: &impl Io,
    record: &Record,
    index: usize,
    level: usize,
    supply: usize,
) -> Result<()> {
    let plan = &record.baseline.plans[index];
    let scheme = plan.scheme()?;
    let before = plan.values[level][supply];
    let current = io.levels(scheme, LEVELS[level])?[supply];
    if current == before {
        return Ok(());
    }
    if current != TARGETS[level] {
        bail!("计划 {} 的阈值已被其他程序修改", plan.id);
    }
    io.write(scheme, LEVELS[level], supply == 0, before)?;
    if io.levels(scheme, LEVELS[level])?[supply] != before {
        bail!("计划 {} 阈值恢复回读不一致", plan.id);
    }
    Ok(())
}
fn restore_cache(io: &impl Io, record: &Record, index: usize) -> Result<()> {
    let current = io.cache()?[index];
    let before = record.baseline.cache[index];
    if current == before {
        return Ok(());
    }
    if current != record.baseline.cache_target()[index] {
        bail!("原厂电池阈值缓存已被其他程序修改");
    }
    io.write_cache(index, before)?;
    if io.cache()?[index] != before {
        bail!("原厂电池阈值缓存恢复回读不一致");
    }
    Ok(())
}
fn restored(value: &Recovery) -> bool {
    value
        .baseline
        .plans
        .iter()
        .enumerate()
        .all(|(index, plan)| {
            (0..3).all(|level| {
                (0..2).all(|supply| {
                    !value.owned[index][level][supply]
                        || value.current[index].values[level][supply] == plan.values[level][supply]
                })
            })
        })
        && (0..3).all(|index| {
            !value.cache_owned[index] || value.cache[index] == value.baseline.cache[index]
        })
}
pub fn restore(
    io: &impl Io,
    store: &FileStore,
    record: &mut Record,
    permit: &mut impl FnMut() -> Result<()>,
) -> Result<()> {
    permit()?;
    io.ensure_writable()?;
    let before = transaction::recovery(io, record)?;
    transaction::compatible(&before)?;
    let active = u128::from_str_radix(&before.current_active, 16)?;
    if record.phase != Phase::Restoring {
        transaction::update(store, record, |r| r.phase = Phase::Restoring)?;
    }
    let mut errors = Vec::new();
    for level in (0..3).rev() {
        for index in (0..record.baseline.plans.len()).rev() {
            for supply in [0, 1] {
                if !record.owned[index][level][supply] {
                    continue;
                }
                guard(io, store, record, active, permit)?;
                if let Err(error) = restore_plan(io, record, index, level, supply) {
                    errors.push(format!(
                        "计划 {}：{error:#}",
                        record.baseline.plans[index].id
                    ));
                }
            }
        }
    }
    if record.baseline.plans.iter().enumerate().any(|(i, p)| {
        p.id == before.current_active && record.owned[i].iter().flatten().any(|owned| *owned)
    }) {
        guard(io, store, record, active, permit)?;
        if let Err(error) = io.activate(active) {
            errors.push(format!("刷新活动计划：{error:#}"));
        }
    }
    for index in (0..3).rev() {
        if !record.cache_owned[index] {
            continue;
        }
        guard(io, store, record, active, permit)?;
        if let Err(error) = restore_cache(io, record, index) {
            errors.push(format!("缓存字段 {index}：{error:#}"));
        }
    }
    guard(io, store, record, active, permit)?;
    if !restored(&transaction::recovery(io, record)?) {
        errors.push("部分初始值尚未恢复".into());
    }
    if !errors.is_empty() {
        bail!("电池阈值恢复未完成：{}", errors.join("；"));
    }
    store.clear(record)
}
