use super::{
    model::*,
    store::FileStore,
    transaction::{self, Io},
};
use anyhow::{Result, bail};

fn guard(
    io: &impl Io,
    store: &FileStore,
    record: &Record,
    permit: &mut impl FnMut() -> Result<()>,
) -> Result<Recovery> {
    permit()?;
    io.ensure_writable()?;
    if store.load()?.as_ref() != Some(record) {
        bail!("低电量亮度恢复记录已变化");
    }
    let view = transaction::recovery(io, record)?;
    transaction::compatible(&view)?;
    Ok(view)
}
fn restore_level(io: &impl Io, record: &Record, view: &Recovery, index: usize) -> Result<()> {
    let before = record.baseline.levels[index];
    if view.levels[index] != before {
        io.write_level(record.baseline.scheme()?, index == 0, before)?;
        if io.levels(record.baseline.scheme()?)?[index] != before {
            bail!("低电量亮度字段恢复回读不一致");
        }
    }
    Ok(())
}
fn restore_cache(io: &impl Io, record: &Record, view: &Recovery, index: usize) -> Result<()> {
    let before = record.baseline.cache[index];
    if view.cache[index] != before {
        io.write_cache(index, before)?;
        if io.cache()?[index] != before {
            bail!("低电量亮度缓存恢复回读不一致");
        }
    }
    Ok(())
}
pub fn restore(
    io: &impl Io,
    store: &FileStore,
    record: &mut Record,
    permit: &mut impl FnMut() -> Result<()>,
) -> Result<()> {
    guard(io, store, record, permit)?;
    if record.phase != Phase::Restoring {
        transaction::update(store, record, |record| record.phase = Phase::Restoring)?;
    }
    let mut errors = Vec::new();
    for index in (0..2).rev() {
        if record.cache_owned[index] {
            let view = guard(io, store, record, permit)?;
            if let Err(error) = restore_cache(io, record, &view, index) {
                errors.push(format!("缓存字段{index}：{error:#}"));
            }
        }
    }
    for index in (0..2).rev() {
        if record.levels_owned[index] {
            let view = guard(io, store, record, permit)?;
            if let Err(error) = restore_level(io, record, &view, index) {
                errors.push(format!("计划亮度字段{index}：{error:#}"));
            }
        }
    }
    if record.levels_owned.iter().any(|owned| *owned) {
        guard(io, store, record, permit)?;
        if let Err(error) = io.activate(record.baseline.scheme()?) {
            errors.push(format!("刷新活动计划：{error:#}"));
        }
    }
    let view = guard(io, store, record, permit)?;
    for index in 0..2 {
        if record.levels_owned[index] && view.levels[index] != record.baseline.levels[index]
            || record.cache_owned[index] && view.cache[index] != record.baseline.cache[index]
        {
            errors.push(format!("字段{index}尚未恢复"));
        }
    }
    if !errors.is_empty() {
        bail!("低电量亮度恢复未完成：{}", errors.join("；"));
    }
    store.clear(record)
}
