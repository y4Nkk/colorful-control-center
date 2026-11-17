//! FnKey FAN 的分模式记忆槽；全速只在当前生效，记忆为自动。
use anyhow::{Result, anyhow, bail};
use ccc_core::settings::{Patch, SettingsAccess};
#[cfg(test)]
mod tests;

pub(crate) fn keeps_mode(caps: &[u8]) -> bool {
    caps.len() >= 22 && caps[..2] == [1, 0] && (caps[20] & 0xD0 != 0 || caps[21] & 3 != 0)
}
fn slot(io: &impl SettingsAccess) -> Result<Option<usize>> {
    if !keeps_mode(&io.read(7, 0, 22).map_err(|e| anyhow!(e))?) {
        return Ok(None);
    }
    let data = io.read(1, 1, 1).map_err(|e| anyhow!(e))?;
    match data.first() {
        Some(0 | 4) => Ok(Some(0)),
        Some(mode @ 1..=3) => Ok(Some(*mode as usize)),
        _ => bail!("当前电源模式不能映射到风扇记忆槽"),
    }
}
pub(crate) fn mode(io: &impl SettingsAccess, mode: u8) -> Result<Vec<Patch>> {
    let mut fields = vec![Patch::new(5, [mode])];
    if let Some(slot) = slot(io)? {
        fields.push(Patch::new(70 + slot, [if mode == 1 { 0 } else { mode }]));
    }
    Ok(fields)
}
pub(crate) fn offset(io: &impl SettingsAccess, offset: u8) -> Result<Vec<Patch>> {
    let mut fields = vec![Patch::new(7, [offset])];
    if let Some(slot) = slot(io)? {
        fields.push(Patch::new(74 + slot, [offset]));
    }
    Ok(fields)
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
