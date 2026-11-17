use super::{
    model::*,
    store::FileStore,
    transaction::{self, Io, LEVELS},
};
use anyhow::{Result, bail};

pub fn write(
    io: &impl Io,
    store: &FileStore,
    record: &mut Record,
    permit: &mut impl FnMut() -> Result<()>,
) -> Result<()> {
    let cache = record.baseline.cache_target();
    for (index, target) in cache.into_iter().enumerate() {
        if target == record.baseline.cache[index] {
            continue;
        }
        transaction::guard(io, store, record, permit)?;
        transaction::update(store, record, |r| r.cache_owned[index] = true)?;
        io.write_cache(index, target)?;
        if io.cache()?[index] != target {
            bail!("原厂电池阈值缓存回读不一致");
        }
    }
    for (level, target) in TARGETS.into_iter().enumerate() {
        for index in 0..record.baseline.plans.len() {
            for supply in [1, 0] {
                if record.baseline.plans[index].values[level][supply] == target {
                    continue;
                }
                transaction::guard(io, store, record, permit)?;
                transaction::update(store, record, |r| r.owned[index][level][supply] = true)?;
                let scheme = record.baseline.plans[index].scheme()?;
                io.write(scheme, LEVELS[level], supply == 0, target)?;
                if io.levels(scheme, LEVELS[level])?[supply] != target {
                    bail!("电池计划阈值回读不一致");
                }
            }
        }
    }
    transaction::guard(io, store, record, permit)?;
    let active = record.baseline.plans[0].scheme()?;
    if io.active()? != active {
        bail!("活动电源计划变化，未刷新旧计划");
    }
    io.activate(active)?;
    transaction::guard(io, store, record, permit)?;
    let snapshot = transaction::snapshot(io)?;
    if snapshot.cache != cache
        || snapshot.plans.iter().any(|plan| {
            TARGETS
                .into_iter()
                .enumerate()
                .any(|(i, target)| plan.values[i] != [target; 2])
        })
    {
        bail!("电池计划阈值最终回读不一致");
    }
    transaction::update(store, record, |r| r.phase = Phase::Applied)
}
