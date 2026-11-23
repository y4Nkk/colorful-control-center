//! FnKey FAN 的分模式记忆槽；全速只在当前生效，记忆为自动。
use super::{FAN_MODE_AUTO, FAN_MODE_FULL_SPEED, FAN_MODE_NOISELESS, NOISELESS_FAN_LABEL};
use crate::powerctl::mode::{self as power, ModeIo};
use anyhow::{Result, anyhow, bail};
use ccc_core::settings::{Patch, SettingsAccess};
#[cfg(test)]
mod tests;

pub(crate) const ENTERTAINMENT_QUIET_OFFSET: usize = 9;

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

pub(super) fn effective_mode(
    io: &impl ModeIo,
    power_mode: u8,
    mode: u8,
    supports_quiet: bool,
) -> Result<u8> {
    if mode == FAN_MODE_AUTO
        && power_mode == 3
        && supports_quiet
        && !power::boost::turbo_enabled(io, power_mode)?
        && entertainment_quiet(io)?
    {
        return Ok(FAN_MODE_NOISELESS);
    }
    Ok(mode)
}

pub(crate) fn keeps_mode(caps: &[u8]) -> bool {
    caps.len() >= 22 && caps[..2] == [1, 0] && (caps[20] & 0xD0 != 0 || caps[21] & 3 != 0)
}
fn slot(io: &impl SettingsAccess) -> Result<Option<usize>> {
    if !keeps_mode(&io.read(7, 0, 22).map_err(|e| anyhow!(e))?) {
        return Ok(None);
    }
    mode_slot(power::current_mode(io)?).map(Some)
}
pub(super) fn mode_slot(mode: u8) -> Result<usize> {
    match mode {
        0 | 4 => Ok(0),
        1..=3 => Ok(mode as usize),
        _ => bail!("当前电源模式不能映射到风扇记忆槽"),
    }
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
        .read(4, 70 + mode_slot(mode)?, 1)
        .map_err(|e| anyhow!(e))?;
    Ok(Some(
        *bytes.first().ok_or_else(|| anyhow!("风扇模式记忆槽为空"))?,
    ))
}
pub(crate) fn mode(io: &impl SettingsAccess, mode: u8) -> Result<Vec<Patch>> {
    let mut fields = vec![Patch::new(5, [mode])];
    if let Some(slot) = slot(io)? {
        let remembered = if mode == FAN_MODE_FULL_SPEED {
            FAN_MODE_AUTO
        } else {
            mode
        };
        fields.push(Patch::new(70 + slot, [remembered]));
    }
    Ok(fields)
}
pub(super) fn offset(slot: Option<usize>, offset: u8) -> Vec<Patch> {
    let mut fields = vec![Patch::new(7, [offset])];
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
        70,
        io.read(4, 70, 8).map_err(|e| anyhow!(e))?,
    )])
}
