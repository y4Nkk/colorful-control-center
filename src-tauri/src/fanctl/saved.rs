//! FnKey FAN 的分模式记忆槽；全速只在当前生效，记忆为自动。
use super::{FAN_MODE_AUTO, FAN_MODE_FULL_SPEED, NOISELESS_FAN_LABEL};
use crate::powerctl::mode as power;
use anyhow::{Result, anyhow, bail};
use ccc_core::settings::{Patch, SettingsAccess};
#[cfg(test)]
mod tests;

pub(super) const MODE_OFFSET: usize = 5;
pub(super) const FAN_OFFSET: usize = 7;
pub(crate) const ENTERTAINMENT_QUIET_OFFSET: usize = 9;
pub(super) const MODE_SLOTS_OFFSET: usize = 70;

pub(crate) fn entertainment_quiet(io: &impl SettingsAccess) -> Result<bool> {
    let value = io
        .read(4, ENTERTAINMENT_QUIET_OFFSET, 1)
        .map_err(|e| anyhow!(e))?;
    match value.as_slice() {
        [0] => Ok(false),
        [1] => Ok(true),
        _ => bail!("娱乐模式的{NOISELESS_FAN_LABEL}偏好未知"),
    }
}

pub(crate) fn keeps_mode(caps: &[u8]) -> bool {
    caps.len() >= 22 && caps[..2] == [1, 0] && (caps[20] & 0xD0 != 0 || caps[21] & 3 != 0)
}
pub(super) fn slot(io: &impl SettingsAccess) -> Result<Option<usize>> {
    if !keeps_mode(&io.read(7, 0, 22).map_err(|e| anyhow!(e))?) {
        return Ok(None);
    }
    mode_slot(power::current_mode(io)?).map(Some)
}
pub(crate) fn mode_slot(mode: u8) -> Result<usize> {
    if !ccc_core::oem_events::is_power_mode(mode) {
        bail!("当前电源模式不能映射到风扇记忆槽");
    }
    Ok(usize::from(mode))
}
pub(crate) fn remembered_mode(
    io: &impl SettingsAccess,
    caps: &[u8],
    mode: u8,
) -> Result<Option<u8>> {
    if !keeps_mode(caps) {
        return Ok(None);
    }
    let bytes = io
        .read(4, MODE_SLOTS_OFFSET + mode_slot(mode)?, 1)
        .map_err(|e| anyhow!(e))?;
    Ok(Some(
        *bytes.first().ok_or_else(|| anyhow!("风扇模式记忆槽为空"))?,
    ))
}
pub(crate) fn mode(io: &impl SettingsAccess, mode: u8) -> Result<Vec<Patch>> {
    let mut fields = vec![Patch::new(MODE_OFFSET, [mode])];
    if let Some(slot) = slot(io)? {
        let remembered = if mode == FAN_MODE_FULL_SPEED {
            FAN_MODE_AUTO
        } else {
            mode
        };
        fields.push(Patch::new(MODE_SLOTS_OFFSET + slot, [remembered]));
    }
    Ok(fields)
}
pub(super) fn offset(slot: Option<usize>, offset: u8) -> Vec<Patch> {
    let mut fields = vec![Patch::new(FAN_OFFSET, [offset])];
    if let Some(slot) = slot {
        fields.push(Patch::new(74 + slot, [offset]));
    }
    fields
}
pub(crate) fn capture_slots(io: &impl SettingsAccess) -> Result<Vec<Patch>> {
    if !keeps_mode(&io.read(7, 0, 22).map_err(|e| anyhow!(e))?) {
        return Ok(vec![]);
    }
    Ok(vec![Patch::new(
        MODE_SLOTS_OFFSET,
        io.read(4, MODE_SLOTS_OFFSET, 8).map_err(|e| anyhow!(e))?,
    )])
}
