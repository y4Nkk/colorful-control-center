use super::{model::*, store::FileStore};
use crate::hal::power_plan::{BatteryLevel, SCHEMES};
use anyhow::{Result, bail};

pub const LEVELS: [BatteryLevel; 3] = [
    BatteryLevel::Low,
    BatteryLevel::Critical,
    BatteryLevel::Reserve,
];
pub trait Io {
    fn active(&self) -> Result<u128>;
    fn schemes(&self) -> Result<Vec<u128>>;
    fn levels(&self, plan: u128, level: BatteryLevel) -> Result<[u32; 2]>;
    fn write(&self, plan: u128, level: BatteryLevel, ac: bool, value: u32) -> Result<()>;
    fn cache(&self) -> Result<[u8; 3]>;
    fn write_cache(&self, index: usize, value: u8) -> Result<()>;
    fn activate(&self, plan: u128) -> Result<()>;
    fn ensure_writable(&self) -> Result<()>;
}
fn read_plan(io: &impl Io, scheme: u128) -> Result<Plan> {
    let mut values = [[0; 2]; 3];
    for (index, level) in LEVELS.into_iter().enumerate() {
        values[index] = io.levels(scheme, level)?;
    }
    Ok(Plan {
        id: format!("{scheme:032x}"),
        values,
    })
}
pub fn snapshot(io: &impl Io) -> Result<Snapshot> {
    let (active, ids) = selected_plans(io)?;
    let plans = ids
        .iter()
        .copied()
        .map(|id| read_plan(io, id))
        .collect::<Result<Vec<_>>>()?;
    let result = Snapshot {
        active: format!("{active:032x}"),
        plans,
        cache: io.cache()?,
    };
    if selected_plans(io)? != (active, ids) {
        bail!("读取期间电源计划范围已变化，请重新预览");
    }
    result.validate()?;
    Ok(result)
}
fn selected_plans(io: &impl Io) -> Result<(u128, Vec<u128>)> {
    let active = io.active()?;
    let schemes = io.schemes()?;
    if !schemes.contains(&active) {
        bail!("活动电源计划不在系统报告中");
    }
    let mut ids = vec![active];
    for id in SCHEMES {
        if id != active && schemes.contains(&id) {
            ids.push(id);
        }
    }
    Ok((active, ids))
}
pub fn recovery(io: &impl Io, record: &Record) -> Result<Recovery> {
    record.validate()?;
    let active = io.active()?;
    let schemes = io.schemes()?;
    if !schemes.contains(&active) {
        bail!("活动电源计划不在系统报告中");
    }
    let mut current = Vec::new();
    for plan in &record.baseline.plans {
        let id = plan.scheme()?;
        if !schemes.contains(&id) {
            bail!("恢复计划 {id:032x} 已不存在，保留恢复记录");
        }
        let found = read_plan(io, id)?;
        if found.values.iter().flatten().any(|value| *value > 100) {
            bail!("恢复计划 {id:032x} 的电池阈值无效");
        }
        current.push(found);
    }
    let cache = io.cache()?;
    if cache.iter().any(|value| *value > 100) {
        bail!("原厂电池阈值缓存无效");
    }
    if io.active()? != active {
        bail!("读取恢复记录期间活动计划已变化");
    }
    let available = io.schemes()?;
    if record
        .baseline
        .plans
        .iter()
        .any(|plan| plan.scheme().map_or(true, |id| !available.contains(&id)))
    {
        bail!("读取恢复记录期间计划集合已变化");
    }
    Ok(Recovery {
        phase: record.phase,
        baseline: record.baseline.clone(),
        current_active: format!("{active:032x}"),
        current,
        cache,
        owned: record.owned.clone(),
        cache_owned: record.cache_owned,
    })
}
pub fn compatible(recovery: &Recovery) -> Result<()> {
    let cache_target = recovery.baseline.cache_target();
    for (index, original) in recovery.baseline.plans.iter().enumerate() {
        let current = &recovery.current[index];
        for (level, target) in TARGETS.into_iter().enumerate() {
            for supply in 0..2 {
                let value = current.values[level][supply];
                if recovery.owned[index][level][supply]
                    && value != original.values[level][supply]
                    && value != target
                {
                    bail!("计划 {} 的电池阈值已被其他程序修改", original.id);
                }
            }
        }
    }
    for (index, target) in cache_target.into_iter().enumerate() {
        if recovery.cache_owned[index]
            && recovery.cache[index] != recovery.baseline.cache[index]
            && recovery.cache[index] != target
        {
            bail!("原厂电池阈值缓存已被其他程序修改");
        }
    }
    Ok(())
}
fn identity(snapshot: &Snapshot, record: Option<&Record>) -> Result<String> {
    let Some(record) = record else {
        return Record::new(snapshot.clone())?.baseline.identity();
    };
    record.validate()?;
    if record.phase != Phase::Applied {
        bail!("存在未完成的电池阈值事务，请先恢复");
    }
    if snapshot.active != record.baseline.active
        || snapshot.plans.len() != record.baseline.plans.len()
    {
        bail!("电池计划集合已变化，请先恢复已接管字段");
    }
    for (index, plan) in snapshot.plans.iter().enumerate() {
        if plan.id != record.baseline.plans[index].id {
            bail!("电池计划集合已变化");
        }
        for (level, target) in TARGETS.into_iter().enumerate() {
            if plan.values[level] != [target; 2] {
                bail!("已接管的电池计划阈值发生变化");
            }
        }
    }
    if snapshot.cache != record.baseline.cache_target() {
        bail!("原厂电池阈值缓存发生变化");
    }
    record.baseline.identity()
}
pub fn preview(io: &impl Io, store: &FileStore) -> Profile {
    let record = store.load();
    let current = snapshot(io);
    let authorized = match (&record, &current) {
        (Ok(record), Ok(current)) => identity(current, record.as_ref()),
        (Err(error), _) | (_, Err(error)) => Err(anyhow::anyhow!("{error:#}")),
    };
    let restore = match &record {
        Ok(Some(record)) => Some(recovery(io, record)),
        Ok(None) => None,
        Err(error) => Some(Err(anyhow::anyhow!("{error:#}"))),
    };
    let restore_view = restore.as_ref().and_then(|r| r.as_ref().ok());
    let restore_check = restore_view.map(compatible);
    let revision = restore_view
        .filter(|_| restore_check.as_ref().is_some_and(Result::is_ok))
        .and_then(|r| super::super::model::digest(r).ok());
    let recovery = restore_view.cloned();
    let restore_error = restore
        .and_then(Result::err)
        .or_else(|| restore_check.and_then(Result::err));
    Profile {
        identity: authorized.as_ref().ok().cloned(),
        current: current.ok(),
        error: authorized.err().map(|error| format!("{error:#}")),
        recovery,
        restore_revision: revision,
        restore_error: restore_error.map(|error| format!("{error:#}")),
    }
}
pub fn guard(
    io: &impl Io,
    store: &FileStore,
    record: &Record,
    permit: &mut impl FnMut() -> Result<()>,
) -> Result<()> {
    permit()?;
    io.ensure_writable()?;
    if store.load()?.as_ref() != Some(record) {
        bail!("电池阈值恢复记录已变化");
    }
    let live = snapshot(io)?;
    if live.active != record.baseline.active || live.plans.len() != record.baseline.plans.len() {
        bail!("电池计划范围已变化，停止后续输出");
    }
    for (index, plan) in live.plans.iter().enumerate() {
        let original = &record.baseline.plans[index];
        if plan.id != original.id {
            bail!("电池计划范围已变化");
        }
        for (level, target) in TARGETS.into_iter().enumerate() {
            for supply in 0..2 {
                let value = plan.values[level][supply];
                if value != original.values[level][supply]
                    && (!record.owned[index][level][supply] || value != target)
                {
                    bail!("电池计划阈值前态已变化");
                }
            }
        }
    }
    let target = record.baseline.cache_target();
    for (index, value) in live.cache.into_iter().enumerate() {
        if value != record.baseline.cache[index]
            && (!record.cache_owned[index] || value != target[index])
        {
            bail!("原厂电池阈值缓存前态已变化");
        }
    }
    Ok(())
}
pub fn update(
    store: &FileStore,
    record: &mut Record,
    change: impl FnOnce(&mut Record),
) -> Result<()> {
    let before = record.clone();
    change(record);
    if let Err(error) = store.save(Some(&before), record) {
        *record = before;
        return Err(error);
    }
    Ok(())
}
pub fn apply(
    io: &impl Io,
    store: &FileStore,
    expected: &str,
    permit: &mut impl FnMut() -> Result<()>,
) -> Result<String> {
    permit()?;
    io.ensure_writable()?;
    let live = snapshot(io)?;
    let existing = store.load()?;
    if identity(&live, existing.as_ref())? != expected {
        bail!("电池阈值授权范围已变化，请重新确认");
    }
    if existing.is_some() {
        return Ok("原厂阈值已设置，本次未重复写入".into());
    }
    let mut record = Record::new(live)?;
    store.save(None, &record)?;
    match super::apply::write(io, store, &mut record, permit) {
        Ok(()) => Ok("低电量 25%、严重电量 20%、预留电量 22% 已写入计划".into()),
        Err(error) => match super::restore::restore(io, store, &mut record, &mut || Ok(())) {
            Ok(()) => Err(error.context("电池阈值设置失败，已恢复本次尝试")),
            Err(restore) => Err(anyhow::anyhow!(
                "电池阈值设置失败：{error:#}；恢复未完成：{restore:#}"
            )),
        },
    }
}
