use super::codec_store::{Record, Store};
use crate::hal::beep_audio::Control;
use anyhow::{Context, Result, ensure};
use serde::Serialize;

// 事件编排持有领域状态与HAL控制锁；该层只复核设备、许可和恢复记录。
pub(super) trait Io {
    fn capture(&self) -> Result<Control>;
    fn write(&self, expected: &Control, muted: bool) -> Result<Control>;
    fn ensure_available(&self) -> Result<()>;
}

#[cfg(not(test))]
pub(super) struct Native;
#[cfg(not(test))]
impl Io for Native {
    fn capture(&self) -> Result<Control> {
        crate::hal::beep_audio::capture()
    }
    fn write(&self, expected: &Control, muted: bool) -> Result<Control> {
        crate::hal::beep_audio::set_muted(expected, muted)
    }
    fn ensure_available(&self) -> Result<()> {
        crate::hal::oem_owner::ensure_owner_absent()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Profile {
    pub current: Option<Control>,
    pub initial: Option<Control>,
    pub error: Option<String>,
    pub restore_revision: Option<String>,
    pub restore_error: Option<String>,
}

pub(super) fn preview(io: &impl Io, store: &Store) -> Profile {
    let loaded = store.load();
    let current = io.capture().and_then(|value| {
        value.validate()?;
        Ok(value)
    });
    let initial = loaded
        .as_ref()
        .ok()
        .and_then(|record| record.as_ref().map(|record| record.initial.clone()));
    let restore_error = loaded
        .as_ref()
        .err()
        .map(|error| format!("{error:#}"))
        .or_else(|| {
            initial.as_ref().and_then(|initial| match &current {
                Ok(current) if current.same_device(initial) => None,
                Ok(_) => Some("原厂PC Beep控件已变化，保留原设备恢复值".into()),
                Err(error) => Some(format!("{error:#}")),
            })
        });
    let restore_revision = if initial.is_some() && restore_error.is_none() {
        super::super::model::digest(&(&initial, current.as_ref().ok())).ok()
    } else {
        None
    };
    Profile {
        initial,
        error: current.as_ref().err().map(|error| format!("{error:#}")),
        current: current.ok(),
        restore_revision,
        restore_error,
    }
}

fn guard(
    io: &impl Io,
    store: &Store,
    record: &Record,
    expected: &Control,
    permit: &mut impl FnMut() -> Result<()>,
) -> Result<()> {
    permit()?;
    io.ensure_available()?;
    store.check(record)?;
    ensure!(
        io.capture()? == *expected,
        "原厂PC Beep控件或静音状态已变化"
    );
    Ok(())
}

pub(super) fn change(
    io: &impl Io,
    store: &Store,
    expected: &Control,
    muted: bool,
    permit: &mut impl FnMut() -> Result<()>,
) -> Result<Control> {
    expected.validate()?;
    permit()?;
    io.ensure_available()?;
    ensure!(io.capture()? == *expected, "原厂PC Beep预览已变化");
    let loaded = store.load()?;
    if let Some(record) = &loaded {
        ensure!(
            record.initial.same_device(expected),
            "存在另一PC Beep控件的恢复值，请先恢复原设备"
        );
    }
    if expected.muted == muted {
        return Ok(expected.clone());
    }
    let created = loaded.is_none();
    let record = loaded.unwrap_or_else(|| Record {
        version: 1,
        initial: expected.clone(),
    });
    if created {
        store.save(None, &record)?;
    }
    if let Err(error) = guard(io, store, &record, expected, permit) {
        let cleared = if created {
            store.clear(&record)
        } else {
            Ok(())
        };
        anyhow::bail!(ccc_core::msr::with_recovery(
            format!("{error:#}"),
            cleared.map_err(|error| format!("撤销未执行音频记录：{error:#}"))
        ))
    }
    let result = io.write(expected, muted).and_then(|after| {
        after.validate()?;
        ensure!(
            after.same_device(expected) && after.muted == muted,
            "原厂PC Beep写后身份或回读不一致"
        );
        ensure!(io.capture()? == after, "原厂PC Beep写后状态已变化");
        store.check(&record)?;
        permit()?;
        Ok(after)
    });
    match result {
        Ok(after) => Ok(after),
        Err(error) => {
            let recovery = compensate(io, expected);
            anyhow::bail!(ccc_core::msr::with_recovery(
                format!("原厂PC Beep设置失败：{error:#}"),
                recovery.map_err(|error| format!("{error:#}"))
            ))
        }
    }
}

fn compensate(io: &impl Io, before: &Control) -> Result<()> {
    io.ensure_available()?;
    let current = io.capture()?;
    ensure!(
        current.same_device(before),
        "PC Beep设备已变化，未恢复其他设备"
    );
    if current.muted != before.muted {
        io.write(&current, before.muted)?;
    }
    ensure!(io.capture()? == *before, "原厂PC Beep补偿回读不一致");
    Ok(())
}

pub(super) fn restore(
    io: &impl Io,
    store: &Store,
    expected: &str,
    permit: &mut impl FnMut() -> Result<()>,
) -> Result<()> {
    permit()?;
    io.ensure_available()?;
    ensure!(
        preview(io, store).restore_revision.as_deref() == Some(expected),
        "原厂PC Beep恢复预览已变化"
    );
    let record = store.load()?.context("没有原厂PC Beep恢复记录")?;
    let current = io.capture()?;
    ensure!(
        record.initial.same_device(&current),
        "原厂PC Beep恢复设备已变化"
    );
    guard(io, store, &record, &current, permit)?;
    if current.muted != record.initial.muted {
        io.write(&current, record.initial.muted)?;
    }
    ensure!(io.capture()? == record.initial, "原厂PC Beep恢复回读不一致");
    store.clear(&record)
}
