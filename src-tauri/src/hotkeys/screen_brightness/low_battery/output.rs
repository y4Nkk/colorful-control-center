use super::{
    model::*,
    rollback,
    store::FileStore,
    transaction::{self, Io},
};
use anyhow::{Result, bail};

pub fn execute(
    io: &impl Io,
    store: &FileStore,
    expected: &str,
    before: Snapshot,
    target: Snapshot,
    signal: Option<u32>,
    permit: &mut impl FnMut() -> Result<()>,
    condition: &mut impl FnMut() -> Result<()>,
) -> Result<()> {
    permit()?;
    io.ensure_writable()?;
    condition()?;
    target.validate()?;
    if transaction::preview(io, store).identity.as_deref() != Some(expected)
        || transaction::snapshot(io)? != before
        || before.identity()? != target.identity()?
    {
        bail!("低电量亮度预览或前态已变化");
    }
    let previous = store.load()?;
    let mut record = previous
        .clone()
        .map(Ok)
        .unwrap_or_else(|| Record::new(before.clone()))?;
    if before == target {
        check_before(io, store, previous.as_ref(), &before, permit, condition)?;
        if let Some(signal) = signal {
            io.signal(signal)?;
        }
        return Ok(());
    }
    record.phase = Phase::Preparing;
    record.pending = Some(Attempt {
        before,
        target,
        levels: [false; 2],
        cache: [false; 2],
    });
    store.save(previous.as_ref(), &record)?;
    let result = (|| {
        check(io, store, &record, permit, condition)?;
        if let Some(signal) = signal {
            io.signal(signal)?;
        }
        write_levels(io, store, &mut record, permit, condition)?;
        write_cache(io, store, &mut record, permit, condition)?;
        check(io, store, &record, permit, condition)?;
        if transaction::snapshot(io)? != record.pending.as_ref().unwrap().target {
            bail!("低电量亮度完成前目标已变化");
        }
        transaction::update(store, &mut record, |record| {
            record.current = record.pending.take().unwrap().target;
            record.phase = Phase::Applied;
        })
    })();
    if let Err(error) = result {
        return match rollback::attempt(io, store, &record, previous.as_ref()) {
            Ok(()) => Err(error.context("低电量亮度操作失败，已恢复本次尝试")),
            Err(restore) => Err(anyhow::anyhow!(
                "低电量亮度操作失败：{error:#}；本次补偿未完成：{restore:#}"
            )),
        };
    }
    Ok(())
}
fn check_before(
    io: &impl Io,
    store: &FileStore,
    previous: Option<&Record>,
    before: &Snapshot,
    permit: &mut impl FnMut() -> Result<()>,
    condition: &mut impl FnMut() -> Result<()>,
) -> Result<()> {
    permit()?;
    io.ensure_writable()?;
    condition()?;
    if store.load()?.as_ref() != previous || transaction::snapshot(io)? != *before {
        bail!("低电量亮度前态已变化");
    }
    Ok(())
}
fn check(
    io: &impl Io,
    store: &FileStore,
    record: &Record,
    permit: &mut impl FnMut() -> Result<()>,
    condition: &mut impl FnMut() -> Result<()>,
) -> Result<()> {
    transaction::guard(io, store, record, permit)?;
    condition()
}
fn write_levels(
    io: &impl Io,
    store: &FileStore,
    record: &mut Record,
    permit: &mut impl FnMut() -> Result<()>,
    condition: &mut impl FnMut() -> Result<()>,
) -> Result<()> {
    for index in 0..2 {
        let attempt = record.pending.as_ref().unwrap();
        let target = attempt.target.levels[index];
        if target == attempt.before.levels[index] {
            continue;
        }
        check(io, store, record, permit, condition)?;
        transaction::update(store, record, |record| {
            record.levels_owned[index] = true;
            record.pending.as_mut().unwrap().levels[index] = true;
        })?;
        check(io, store, record, permit, condition)?;
        io.write_level(record.baseline.scheme()?, index == 0, target)?;
        if io.levels(record.baseline.scheme()?)?[index] != target {
            bail!("低电量计划亮度回读不一致");
        }
        check(io, store, record, permit, condition)?;
        io.activate(record.baseline.scheme()?)?;
    }
    Ok(())
}
fn write_cache(
    io: &impl Io,
    store: &FileStore,
    record: &mut Record,
    permit: &mut impl FnMut() -> Result<()>,
    condition: &mut impl FnMut() -> Result<()>,
) -> Result<()> {
    for index in 0..2 {
        let attempt = record.pending.as_ref().unwrap();
        let target = attempt.target.cache[index];
        if target == attempt.before.cache[index] {
            continue;
        }
        check(io, store, record, permit, condition)?;
        transaction::update(store, record, |record| {
            record.cache_owned[index] = true;
            record.pending.as_mut().unwrap().cache[index] = true;
        })?;
        check(io, store, record, permit, condition)?;
        io.write_cache(index, target)?;
        if io.cache()?[index] != target {
            bail!("低电量亮度缓存回读不一致");
        }
    }
    Ok(())
}
