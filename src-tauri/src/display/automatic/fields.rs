use crate::display::Mode;
use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use std::{io::Read, path::PathBuf};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Attempt {
    index: usize,
    value: u8,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Transition {
    pub before: Mode,
    pub target: Mode,
    pub mode: u8,
    pub on_ac: Option<bool>,
    pub previous_cache: u8,
    pub target_cache: u8,
}
impl Transition {
    fn validate(&self) -> Result<()> {
        self.before.validate()?;
        self.target.validate()?;
        let mut original = self.target.clone();
        original.frequency = self.before.frequency;
        if original != self.before || !ccc_core::oem_events::is_power_mode(self.mode) {
            bail!("自动刷新率事务只能改变刷新率");
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Record {
    version: u8,
    active: bool,
    identity: String,
    initial: [u8; 3],
    current: [u8; 3],
    owned: [bool; 3],
    initial_display: Mode,
    owned_display: Option<Mode>,
    attempt: Option<Attempt>,
    transition: Option<Transition>,
}
impl Record {
    pub fn new(identity: String, initial: [u8; 3], initial_display: Mode) -> Result<Self> {
        let record = Self {
            version: 1,
            active: false,
            identity,
            initial,
            current: initial,
            owned: [false; 3],
            initial_display,
            owned_display: None,
            attempt: None,
            transition: None,
        };
        record.validate()?;
        Ok(record)
    }
    pub fn active(&self) -> bool {
        self.active
    }
    pub fn identity(&self) -> &str {
        &self.identity
    }
    pub fn current(&self) -> [u8; 3] {
        self.current
    }
    pub fn initial(&self) -> [u8; 3] {
        self.initial
    }
    pub fn initial_display(&self) -> &Mode {
        &self.initial_display
    }
    pub fn owned_display(&self) -> Option<&Mode> {
        self.owned_display.as_ref()
    }
    pub fn interrupted(&self) -> bool {
        self.attempt.is_some()
    }
    pub fn transition(&self) -> Option<&Transition> {
        self.transition.as_ref()
    }
    fn validate(&self) -> Result<()> {
        self.initial_display.validate()?;
        if self.version != 1
            || self.identity.len() != 64
            || !self
                .identity
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            || self.initial[..2].iter().any(|value| *value > 1)
            || self.current[..2].iter().any(|value| *value > 1)
            || self
                .attempt
                .as_ref()
                .is_some_and(|attempt| attempt.index >= 3 || attempt.index < 2 && attempt.value > 1)
        {
            bail!("自动刷新率恢复记录无效");
        }
        if (0..3).any(|index| !self.owned[index] && self.current[index] != self.initial[index]) {
            bail!("自动刷新率字段所有权无效");
        }
        if let Some(transition) = &self.transition {
            transition.validate()?;
            if !same_display(&self.initial_display, &transition.before) {
                bail!("自动刷新率事务显示器不一致");
            }
            if self.current[2] != transition.previous_cache
                && self.current[2] != transition.target_cache
            {
                bail!("自动刷新率事务缓存状态无效");
            }
        }
        if let Some(mode) = &self.owned_display {
            mode.validate()?;
            if !same_display(&self.initial_display, mode) {
                bail!("自动刷新率恢复显示器不一致");
            }
        }
        Ok(())
    }
}
fn same_display(first: &Mode, second: &Mode) -> bool {
    let mut normalized = second.clone();
    normalized.frequency = first.frequency;
    first.same_state(&normalized)
}

#[derive(Clone)]
pub(super) struct FileStore(pub PathBuf);
impl FileStore {
    pub fn load(&self) -> Result<Option<Record>> {
        let file = match std::fs::File::open(&self.0) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error).context("读取自动刷新率恢复记录"),
        };
        let mut bytes = Vec::new();
        file.take(65_537).read_to_end(&mut bytes)?;
        if bytes.len() > 65_536 {
            bail!("自动刷新率恢复记录过大");
        }
        let record: Record = serde_json::from_slice(&bytes).context("自动刷新率恢复记录损坏")?;
        record.validate()?;
        Ok(Some(record))
    }
    pub fn save(&self, expected: Option<&Record>, next: &Record) -> Result<()> {
        next.validate()?;
        if self.load()?.as_ref() != expected {
            bail!("自动刷新率恢复记录已变化");
        }
        std::fs::create_dir_all(self.0.parent().context("自动刷新率记录目录无效")?)?;
        crate::storage::write(&self.0, &serde_json::to_vec_pretty(next)?)?;
        Ok(())
    }
    fn clear(&self, expected: &Record) -> Result<()> {
        if self.load()?.as_ref() != Some(expected) {
            bail!("自动刷新率恢复记录已变化");
        }
        std::fs::remove_file(&self.0).context("删除自动刷新率恢复记录")
    }
}
pub(super) trait Io {
    fn identity(&self) -> Result<String>;
    fn fields(&self) -> Result<[u8; 3]>;
    fn write(&self, index: usize, value: u8) -> Result<()>;
}
fn update(store: &FileStore, record: &mut Record, change: impl FnOnce(&mut Record)) -> Result<()> {
    let mut next = record.clone();
    change(&mut next);
    store.save(Some(record), &next)?;
    *record = next;
    Ok(())
}
fn guard(
    io: &impl Io,
    store: &FileStore,
    record: &Record,
    permit: &mut impl FnMut() -> Result<()>,
) -> Result<[u8; 3]> {
    permit()?;
    if store.load()?.as_ref() != Some(record) {
        bail!("自动刷新率恢复记录已变化");
    }
    if io.identity()? != record.identity {
        bail!("自动刷新率设备来源已变化");
    }
    io.fields()
}

pub(super) fn verify(
    io: &impl Io,
    store: &FileStore,
    record: &Record,
    permit: &mut impl FnMut() -> Result<()>,
) -> Result<()> {
    if !record.active || record.attempt.is_some() {
        bail!("自动刷新率未启用或事务未完成");
    }
    confirm(io, store, record, permit)
}
pub(super) fn confirm(
    io: &impl Io,
    store: &FileStore,
    record: &Record,
    permit: &mut impl FnMut() -> Result<()>,
) -> Result<()> {
    if record.attempt.is_some() {
        bail!("自动刷新率事务未完成");
    }
    if guard(io, store, record, permit)? != record.current {
        bail!("原厂自动刷新率字段已被修改");
    }
    Ok(())
}
pub(super) fn set_active(store: &FileStore, record: &mut Record, active: bool) -> Result<()> {
    if active && (record.attempt.is_some() || record.transition.is_some()) {
        bail!("自动刷新率事务尚未恢复");
    }
    update(store, record, |record| record.active = active)
}

pub(super) fn begin_transition(
    io: &impl Io,
    store: &FileStore,
    record: &mut Record,
    transition: Transition,
    permit: &mut impl FnMut() -> Result<()>,
) -> Result<()> {
    transition.validate()?;
    if record.transition.is_some() || record.current[2] != transition.previous_cache {
        bail!("自动刷新率事务或缓存已变化");
    }
    verify(io, store, record, permit)?;
    update(store, record, |record| record.transition = Some(transition))
}

pub(super) fn finish_transition(
    io: &impl Io,
    store: &FileStore,
    record: &mut Record,
    kept: bool,
    permit: &mut impl FnMut() -> Result<()>,
) -> Result<()> {
    let transition = record
        .transition
        .clone()
        .ok_or_else(|| anyhow!("没有待完成的自动刷新率事务"))?;
    confirm(io, store, record, permit)?;
    update(store, record, |record| {
        if kept && !transition.before.same_state(&transition.target) {
            record.owned_display = (!transition.target.same_state(&record.initial_display))
                .then_some(transition.target);
        }
        record.transition = None;
    })
}
pub(super) fn clear_owned_display(
    io: &impl Io,
    store: &FileStore,
    record: &mut Record,
    permit: &mut impl FnMut() -> Result<()>,
) -> Result<()> {
    if record.transition.is_some() {
        bail!("自动刷新率显示事务尚未恢复");
    }
    confirm(io, store, record, permit)?;
    update(store, record, |record| record.owned_display = None)
}

pub(super) fn change(
    io: &impl Io,
    store: &FileStore,
    record: &mut Record,
    index: usize,
    target: u8,
    permit: &mut impl FnMut() -> Result<()>,
) -> Result<()> {
    if index >= 3 || index < 2 && target > 1 || record.attempt.is_some() {
        bail!("自动刷新率字段或中间状态无效");
    }
    let fields = guard(io, store, record, permit)?;
    if fields != record.current {
        bail!("原厂刷新率字段已变化，请重新预览");
    }
    if fields[index] == target {
        return Ok(());
    }
    update(store, record, |record| {
        record.attempt = Some(Attempt {
            index,
            value: target,
        })
    })?;
    let result = (|| {
        let fields = guard(io, store, record, permit)?;
        if fields != record.current {
            bail!("原厂刷新率字段在写入前变化");
        }
        io.write(index, target)?;
        if io.fields()?[index] != target {
            bail!("原厂刷新率字段回读不一致");
        }
        update(store, record, |record| {
            record.current[index] = target;
            record.owned[index] = true;
            record.attempt = None;
        })
    })();
    if let Err(error) = result {
        let recovery = rollback_attempt(io, store, record, &mut || Ok(()));
        return Err(anyhow!(ccc_core::msr::with_recovery(
            format!("{error:#}"),
            recovery.map_err(|error| format!("{error:#}"))
        )));
    }
    Ok(())
}
pub(super) fn rollback_attempt(
    io: &impl Io,
    store: &FileStore,
    record: &mut Record,
    permit: &mut impl FnMut() -> Result<()>,
) -> Result<()> {
    let Some(attempt) = record.attempt.clone() else {
        return Ok(());
    };
    let fields = guard(io, store, record, permit)?;
    let old = record.current[attempt.index];
    if fields[attempt.index] != old && fields[attempt.index] != attempt.value {
        bail!("原厂刷新率字段被外部修改，保留恢复记录");
    }
    if fields[attempt.index] != old {
        io.write(attempt.index, old)?;
        if io.fields()?[attempt.index] != old {
            bail!("原厂刷新率回滚回读不一致");
        }
    }
    update(store, record, |record| record.attempt = None)
}
pub(super) fn restore(
    io: &impl Io,
    store: &FileStore,
    record: &mut Record,
    permit: &mut impl FnMut() -> Result<()>,
) -> Result<()> {
    if record.transition.is_some() {
        bail!("请先恢复自动刷新率显示事务");
    }
    rollback_attempt(io, store, record, permit)?;
    let mut errors = Vec::new();
    for index in (0..3).rev() {
        if !record.owned[index] {
            continue;
        }
        let result = (|| {
            let fields = guard(io, store, record, permit)?;
            if fields[index] == record.initial[index] {
                return Ok(());
            }
            if fields[index] != record.current[index] {
                bail!("字段已被外部修改");
            }
            update(store, record, |record| {
                record.attempt = Some(Attempt {
                    index,
                    value: record.initial[index],
                })
            })?;
            let fields = guard(io, store, record, permit)?;
            if fields[index] != record.current[index] {
                bail!("字段在恢复前变化");
            }
            io.write(index, record.initial[index])?;
            if io.fields()?[index] != record.initial[index] {
                bail!("字段恢复回读不一致");
            }
            update(store, record, |record| {
                record.current[index] = record.initial[index];
                record.attempt = None;
            })
        })();
        if let Err(error) = result {
            errors.push(format!("字段{}：{error:#}", index + 27));
            break;
        }
    }
    if !errors.is_empty() {
        bail!("自动刷新率恢复未完成：{}", errors.join("；"));
    }
    store.clear(record)
}
