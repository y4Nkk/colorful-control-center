use super::Change;
use crate::display::{Access, Mode};
use anyhow::{Result, anyhow, bail};

pub(crate) fn matches(io: &dyn Access, expected: &[Mode]) -> Result<bool> {
    let actual = io.list()?;
    Ok(expected.iter().all(|mode| {
        actual
            .iter()
            .any(|display| display.current.same_state(mode))
    }))
}

pub(crate) fn restore(io: &dyn Access, changes: &[Change]) -> Result<()> {
    let mut errors = Vec::new();
    for change in changes.iter().rev() {
        if let Err(error) = restore_one(io, change) {
            errors.push(format!("{}：{error:#}", change.before.name));
        }
    }
    if !errors.is_empty() {
        bail!(errors.join("；"));
    }
    Ok(())
}

fn restore_one(io: &dyn Access, change: &Change) -> Result<()> {
    let actual = io
        .list()?
        .into_iter()
        .find(|display| display.current.device == change.before.device)
        .ok_or_else(|| anyhow!("原显示器已断开，保留恢复资格"))?;
    if actual.current.same_state(&change.before) {
        return Ok(());
    }
    if !actual.current.same_state(&change.target) {
        bail!("显示模式已被外部修改或设备替换，未覆盖当前模式");
    }
    io.set(&change.before)?;
    if !matches(io, std::slice::from_ref(&change.before))? {
        bail!("显示恢复回读与本次前态不一致");
    }
    Ok(())
}
