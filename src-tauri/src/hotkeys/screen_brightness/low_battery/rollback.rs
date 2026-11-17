use super::{
    model::*,
    store::FileStore,
    transaction::{self, Io},
};
use anyhow::{Context, Result, bail};

pub fn attempt(
    io: &impl Io,
    store: &FileStore,
    record: &Record,
    previous: Option<&Record>,
) -> Result<()> {
    let pending = record.pending.as_ref().context("低电量亮度尝试记录缺失")?;
    let mut errors = Vec::new();
    for index in (0..2).rev() {
        if pending.cache[index] {
            let result = (|| {
                transaction::guard(io, store, record, &mut || Ok(()))?;
                let before = pending.before.cache[index];
                if io.cache()?[index] != before {
                    io.write_cache(index, before)?;
                }
                if io.cache()?[index] != before {
                    bail!("缓存补偿回读不一致");
                }
                Ok(())
            })();
            if let Err(error) = result {
                errors.push(format!("缓存{index}：{error:#}"));
            }
        }
    }
    for index in (0..2).rev() {
        if pending.levels[index] {
            let result = restore_level(io, store, record, &pending.before, index);
            if let Err(error) = result {
                errors.push(format!("计划亮度{index}：{error:#}"));
            }
        }
    }
    if !errors.is_empty() {
        bail!("{}", errors.join("；"));
    }
    transaction::guard(io, store, record, &mut || Ok(()))?;
    if transaction::snapshot(io)? != pending.before {
        bail!("本次亮度补偿后前态不一致");
    }
    match previous {
        Some(previous) => store.save(Some(record), previous),
        None => store.clear(record),
    }
}
fn restore_level(
    io: &impl Io,
    store: &FileStore,
    record: &Record,
    before: &Snapshot,
    index: usize,
) -> Result<()> {
    transaction::guard(io, store, record, &mut || Ok(()))?;
    let target = before.levels[index];
    let plan = before.scheme()?;
    if io.levels(plan)?[index] != target {
        io.write_level(plan, index == 0, target)?;
    }
    if io.levels(plan)?[index] != target {
        bail!("计划亮度补偿回读不一致");
    }
    transaction::guard(io, store, record, &mut || Ok(()))?;
    io.activate(plan)
}
