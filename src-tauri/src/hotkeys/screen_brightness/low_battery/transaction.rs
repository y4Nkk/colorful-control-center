use super::{model::*, store::FileStore};
use crate::battery::PowerStatus;
use anyhow::{Context, Result, bail};

pub trait Io {
    fn known_firmware(&self) -> bool;
    fn rule(&self) -> Result<Rule>;
    fn panel(&self) -> Result<String>;
    fn active(&self) -> Result<u128>;
    fn levels(&self, plan: u128) -> Result<[u32; 2]>;
    fn write_level(&self, plan: u128, ac: bool, value: u32) -> Result<()>;
    fn cache(&self) -> Result<[u8; 2]>;
    fn write_cache(&self, index: usize, value: u8) -> Result<()>;
    fn power(&self) -> Result<PowerStatus>;
    fn signal(&self, value: u32) -> Result<()>;
    fn activate(&self, plan: u128) -> Result<()>;
    fn ensure_writable(&self) -> Result<()>;
}

pub fn power(io: &impl Io) -> Result<(bool, u8)> {
    power_values(&io.power()?)
}
fn power_values(power: &PowerStatus) -> Result<(bool, u8)> {
    if power.present != Some(true) {
        bail!("未确认电池存在");
    }
    let online = power.ac_online.context("供电状态未知")?;
    let percent = power.percent.context("电池电量未知")?;
    if percent > 100 {
        bail!("电池电量超出百分比范围");
    }
    Ok((online, percent))
}
pub fn should_restore(io: &impl Io, saved_percent: u8) -> Result<bool> {
    let power = io.power()?;
    if power.ac_online == Some(true) {
        return Ok(true);
    }
    let (online, percent) = power_values(&power)?;
    Ok(online || percent > saved_percent)
}
pub fn snapshot(io: &impl Io) -> Result<Snapshot> {
    if !io.known_firmware() {
        bail!("当前固件未经低电量通知静态核对");
    }
    let active = io.active()?;
    let result = Snapshot {
        rule: io.rule()?,
        panel: io.panel()?,
        plan: format!("{active:032x}"),
        levels: io.levels(active)?,
        cache: io.cache()?,
    };
    if io.active()? != active {
        bail!("读取低电量亮度期间活动计划已变化");
    }
    result.validate()?;
    Ok(result)
}
pub fn recovery(io: &impl Io, record: &Record) -> Result<Recovery> {
    record.validate()?;
    if !io.known_firmware() {
        bail!("原固件已变化，保留低电量亮度恢复记录");
    }
    if io.rule()? != record.baseline.rule {
        bail!("原厂低电量亮度能力已变化，保留恢复记录");
    }
    let active = io.active()?;
    let result = Recovery {
        record: record.clone(),
        active: format!("{active:032x}"),
        panel: io.panel()?,
        levels: io.levels(record.baseline.scheme()?)?,
        cache: io.cache()?,
    };
    if io.active()? != active {
        bail!("读取低电量亮度恢复状态时计划已变化");
    }
    Ok(result)
}
pub fn compatible(view: &Recovery) -> Result<()> {
    let record = &view.record;
    if view.active != record.baseline.plan || view.panel != record.baseline.panel {
        bail!("活动计划或面板已变化，保留低电量亮度恢复记录");
    }
    for index in 0..2 {
        let level = view.levels[index];
        let cached = view.cache[index];
        let (levels, cache) = accepted(record, index);
        if level > 100
            || cached > 100
            || (record.levels_owned[index] || record.pending.is_some()) && !levels.contains(&level)
            || (record.cache_owned[index] || record.pending.is_some()) && !cache.contains(&cached)
        {
            bail!("低电量亮度或缓存已被其他程序修改");
        }
    }
    Ok(())
}
fn accepted(record: &Record, index: usize) -> (Vec<u32>, Vec<u8>) {
    let (mut levels, mut cache) = if let Some(attempt) = &record.pending {
        let mut levels = vec![attempt.before.levels[index]];
        let mut cache = vec![attempt.before.cache[index]];
        if attempt.levels[index] {
            levels.push(attempt.target.levels[index]);
        }
        if attempt.cache[index] {
            cache.push(attempt.target.cache[index]);
        }
        (levels, cache)
    } else {
        (
            vec![record.current.levels[index]],
            vec![record.current.cache[index]],
        )
    };
    if record.phase == Phase::Restoring {
        if record.levels_owned[index] {
            levels.push(record.baseline.levels[index]);
        }
        if record.cache_owned[index] {
            cache.push(record.baseline.cache[index]);
        }
    }
    (levels, cache)
}
fn identity(current: &Snapshot, record: Option<&Record>) -> Result<String> {
    let Some(record) = record else {
        return current.identity();
    };
    if record.phase != Phase::Applied || record.pending.is_some() {
        bail!("低电量亮度事务未完成，请先恢复");
    }
    if current.identity()? != record.baseline.identity()? || current.cache != record.current.cache {
        bail!("低电量亮度来源或缓存已变化，请先恢复");
    }
    // 原厂设置和自然结束均读取当时的AC亮度；手动初始恢复另作字段冲突检查。
    current.identity()
}
pub fn preview(io: &impl Io, store: &FileStore) -> Profile {
    let record = store.load();
    let current = snapshot(io);
    let power = io.power();
    let authorized = match (&record, &current) {
        (Ok(Some(record)), Ok(current)) => identity(current, Some(record)),
        (Ok(None), Ok(current)) => identity(current, None),
        (Err(error), _) | (_, Err(error)) => Err(anyhow::anyhow!("{error:#}")),
    };
    let restore = match &record {
        Ok(Some(record)) => Some(recovery(io, record)),
        Ok(None) => None,
        Err(error) => Some(Err(anyhow::anyhow!("{error:#}"))),
    };
    let view = restore.as_ref().and_then(|value| value.as_ref().ok());
    let check = view.map(compatible);
    Profile {
        identity: authorized.as_ref().ok().cloned(),
        setting_target: current.as_ref().ok().zip(power.as_ref().ok()).and_then(
            |(current, power)| {
                power_values(power)
                    .ok()
                    .and_then(|(online, percent)| {
                        if online {
                            Some(current.clone())
                        } else {
                            current.setting(percent).ok()
                        }
                    })
                    .map(|target| target.levels)
            },
        ),
        ending_target: current
            .as_ref()
            .ok()
            .and_then(|current| current.ending().ok())
            .map(|target| target.levels),
        current: current.ok(),
        on_ac: power.as_ref().ok().and_then(|value| match value.ac_online {
            Some(false) if value.present != Some(true) => None,
            state => state,
        }),
        percent: power
            .as_ref()
            .ok()
            .and_then(|value| value.percent.filter(|percent| *percent <= 100)),
        error: authorized.err().map(|error| format!("{error:#}")),
        recovery: view.cloned(),
        restore_revision: view
            .filter(|_| check.as_ref().is_some_and(Result::is_ok))
            .and_then(|value| super::super::super::model::digest(value).ok()),
        restore_error: restore
            .and_then(Result::err)
            .or_else(|| check.and_then(Result::err))
            .map(|error| format!("{error:#}")),
    }
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
pub fn guard(
    io: &impl Io,
    store: &FileStore,
    record: &Record,
    permit: &mut impl FnMut() -> Result<()>,
) -> Result<()> {
    permit()?;
    io.ensure_writable()?;
    if store.load()?.as_ref() != Some(record) {
        bail!("低电量亮度恢复记录已变化");
    }
    let current = snapshot(io)?;
    if current.identity()? != record.baseline.identity()? {
        bail!("低电量亮度来源变化，停止后续输出");
    }
    let view = recovery(io, record)?;
    compatible(&view)?;
    Ok(())
}
