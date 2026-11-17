mod apply;
mod model;
mod native;
mod output;
mod restore;
mod rollback;
mod store;
mod transaction;

use anyhow::{Result, bail};
pub(crate) use model::Profile;
pub(super) use native::Native;
pub(super) use store::FileStore;
use transaction::Io;

pub(super) fn preview(io: &impl Io, store: &FileStore) -> Profile {
    let mut view = transaction::preview(io, store);
    if view
        .current
        .as_ref()
        .is_some_and(|current| !current.rule.low_supported())
    {
        view.identity = None;
        view.error = Some("原厂未声明低电量亮度控制".into());
    } else if view.recovery.is_none() {
        if let Err(error) = transaction::power(io) {
            view.identity = None;
            view.error = Some(format!("{error:#}"));
        }
    }
    view
}
pub(super) fn preview_alert(io: &impl Io, store: &FileStore) -> Profile {
    transaction::preview(io, store)
}
pub(super) fn absolute(
    io: &impl Io,
    store: &FileStore,
    expected: &str,
    value: u8,
    permit: &mut impl FnMut() -> Result<()>,
) -> Result<()> {
    if value > 100 {
        bail!("原厂电池事件亮度超出百分比范围");
    }
    let before = transaction::snapshot(io)?;
    let mut target = before.clone();
    target.levels = [u32::from(value); 2];
    output::execute(
        io,
        store,
        expected,
        before,
        target,
        None,
        permit,
        &mut || Ok(()),
    )
}
pub(super) fn alert_low(
    io: &impl Io,
    store: &FileStore,
    expected: &str,
    permit: &mut impl FnMut() -> Result<()>,
) -> Result<()> {
    if !transaction::power(io)?.0 {
        apply::apply(io, store, expected, false, permit)?;
    }
    Ok(())
}
pub(super) fn event(
    io: &impl Io,
    store: &FileStore,
    expected: &str,
    event: u8,
    permit: &mut impl FnMut() -> Result<()>,
) -> Result<String> {
    if event == 59 {
        if transaction::power(io)?.0 {
            return Ok("已接入电源，未执行低电量调暗".into());
        }
        return apply::apply(io, store, expected, true, permit);
    }
    if event != 60 {
        bail!("未知低电量亮度事件");
    }
    if !transaction::snapshot(io)?.rule.low_supported() {
        bail!("原厂未声明低电量亮度控制");
    }
    apply::finish(io, store, expected, true, permit)?;
    Ok("已按原厂规则结束低电量亮度，首次恢复值仍保留".into())
}
pub(super) fn check(
    io: &impl Io,
    store: &FileStore,
    expected: &str,
    permit: &mut impl FnMut() -> Result<()>,
) -> Result<Option<String>> {
    let current = transaction::snapshot(io)?;
    if !current.rule.low_supported() || current.cache[0] == 0 {
        return Ok(None);
    }
    if transaction::preview(io, store).identity.as_deref() != Some(expected) {
        bail!("低电量亮度来源已变化，保留恢复记录");
    }
    if !transaction::should_restore(io, current.cache[1])? {
        return Ok(None);
    }
    apply::finish(io, store, expected, false, permit)?;
    Ok(Some("已按原厂规则恢复亮度，首次设置仍可单独恢复".into()))
}
pub(super) fn restore(io: &impl Io, store: &FileStore, expected: &str) -> Result<()> {
    let view = transaction::preview(io, store);
    if view.restore_revision.as_deref() != Some(expected) {
        bail!("低电量亮度恢复预览已变化，请重新查看");
    }
    let Some(mut record) = store.load()? else {
        bail!("没有待恢复的低电量亮度记录");
    };
    restore::restore(io, store, &mut record, &mut || Ok(()))
}

#[cfg(test)]
mod native_tests;
#[cfg(test)]
mod tests;
