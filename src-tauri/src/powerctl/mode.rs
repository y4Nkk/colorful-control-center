//! OEM FnKey.Features.SetPowerMode 协议；page1 是保存值，cmd121/sub25 是即时命令。
use crate::hal::{Hal, oem_dchu::Dchu};
use anyhow::{Result, anyhow, bail};
use ccc_core::settings::{self, Patch, SettingsAccess};
use serde::Serialize;
pub mod boost;
pub(crate) mod cycle;
mod energy;
pub(crate) mod links;
pub use energy::State as EnergyState;
mod policy;
pub(crate) mod profile;
pub(crate) mod restoration;
pub(crate) mod selection;
pub(crate) mod supply;
mod support;
mod system;
mod workflow;
pub use policy::Target;
#[cfg(test)]
pub(crate) use workflow::checkpoint_for_profile;
#[cfg(test)]
pub use workflow::restore;
pub use workflow::{Plan, Snapshot, restore_checkpoint};
#[cfg(test)]
pub use workflow::{apply_preview, checkpoint};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CoreSnapshot {
    mode: u8,
    command: u32,
    preferences: [u8; 2],
    ac_mode: Option<u8>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModeProfile {
    pub current_mode: u8,
    pub ac_mode: u8,
    pub energy: Option<EnergyState>,
    pub restore: Option<workflow::RestoreTarget>,
    pub restore_blocked_reason: Option<String>,
    pub available_modes: Vec<u8>,
    pub can_restore: bool,
    pub targets: Vec<Target>,
    pub software_active: bool,
    pub blocked_reason: Option<String>,
}

pub(crate) trait ModeIo: SettingsAccess {
    fn feature_flags(&self, command: i32) -> Result<u32>;
    fn send_mode(&self, command: u32) -> Result<()>;
    fn fan_info(&self) -> Result<[u8; 256]>;
    fn table(&self, selector: u8) -> Result<[u8; 256]>;
}

impl ModeIo for Dchu {
    fn feature_flags(&self, command: i32) -> Result<u32> {
        let (valid, flags) = self.read_int(command);
        if !valid {
            bail!("未能读取原厂WMI{command}基础能力，不能判定为固件不支持");
        }
        Ok(flags as u32)
    }
    fn fan_info(&self) -> Result<[u8; 256]> {
        self.read_package(13)
    }
    fn table(&self, selector: u8) -> Result<[u8; 256]> {
        let mut packet = [0; 256];
        packet[0] = selector;
        self.exchange_package(4, &packet)
    }
    fn send_mode(&self, command: u32) -> Result<()> {
        self.set_data_sub(121, 25, command)
    }
}

fn byte(io: &impl SettingsAccess, page: u8, offset: usize) -> Result<u8> {
    let bytes = io.read(page, offset, 1).map_err(|e| anyhow!(e))?;
    bytes
        .first()
        .copied()
        .ok_or_else(|| anyhow!("模式字段为空"))
}

fn capture(io: &impl SettingsAccess, remember_ac: bool) -> Result<CoreSnapshot> {
    let mode = byte(io, 1, 1)?;
    if mode > 4 {
        bail!("当前 OEM 模式 {mode} 不在已核对的 0–4 范围");
    }
    let preferences = [byte(io, 4, 8)?, byte(io, 1, 32)?];
    if preferences.iter().any(|value| *value > 1) {
        bail!("未知Turbo风扇或DTT保存值");
    }
    Ok(CoreSnapshot {
        mode,
        command: mode_command(mode, preferences),
        preferences,
        ac_mode: remember_ac.then(|| byte(io, 1, 16)).transpose()?,
    })
}

fn mode_command(mode: u8, preferences: [u8; 2]) -> u32 {
    u32::from(if mode == 4 { 0 } else { mode })
        | if mode == 2 {
            u32::from(preferences[0]) << 6 | u32::from(preferences[1]) << 7
        } else {
            0
        }
}

pub(crate) fn available(io: &impl ModeIo) -> Result<Vec<u8>> {
    let caps = io.read(7, 0, 18).map_err(|e| anyhow!(e))?;
    if caps.len() != 18 {
        bail!("BIOS模式能力长度不足");
    }
    let header = &caps[..2];
    if header != [0, 0] && header != [1, 0] {
        bail!("未知 OEM 能力页版本，未开放模式写入");
    }
    let bits = caps[17];
    if bits & 15 == 0 {
        return Ok(if feature_flags(io, &caps, 16)? & 1 != 0 {
            vec![0, 1, 2, 3]
        } else {
            vec![]
        });
    }
    let extended = header == [1, 0];
    let mut modes: Vec<_> = (0..4)
        .filter(|mode| bits & (1 << mode) != 0 && (*mode != 0 || !extended || bits & 0x80 == 0))
        .collect();
    if extended && bits & 0x80 != 0 && bits & 15 != 0 {
        modes.push(4);
    }
    Ok(modes)
}

fn feature_flags(io: &impl ModeIo, caps: &[u8], command: i32) -> Result<u32> {
    let word = |at| u32::from_le_bytes(caps[at..at + 4].try_into().unwrap());
    let short = |at| u16::from_le_bytes(caps[at..at + 2].try_into().unwrap()) as u32;
    if caps[..2] == [1, 0]
        && word(2)
            .wrapping_add(word(6))
            .wrapping_add(short(10))
            .wrapping_add(short(13))
            != 0
    {
        return match command {
            16 => Ok(short(13)),
            96 => Ok(short(10)),
            _ => bail!("未实现此BIOS模式能力字段"),
        };
    }
    io.feature_flags(command)
}

pub fn read(hal: &Hal) -> Result<ModeProfile> {
    let mut profile = hal.with_dchu(|io| {
        let support_issue = support::issue(io)?;
        let modes = if support_issue.is_some() {
            Vec::new()
        } else {
            available(io)?
        };
        let (energy, targets) = if modes.is_empty() {
            (None, Vec::new())
        } else {
            let state = energy::read(io, hal.reviewed_dsdt)?;
            let targets = modes
                .iter()
                .map(|mode| policy::target(io, *mode, state))
                .collect::<Result<Vec<_>>>()?;
            (Some(state), targets)
        };
        Ok(ModeProfile {
            current_mode: byte(io, 1, 1)?,
            ac_mode: byte(io, 1, 16)?,
            energy,
            restore: None,
            restore_blocked_reason: None,
            available_modes: modes,
            can_restore: false,
            software_active: false,
            blocked_reason: support_issue.or_else(|| {
                hal.ensure_mode_change_allowed()
                    .err()
                    .map(|e| format!("{e:#}"))
            }),
            targets,
        })
    })?;
    profile.restore = workflow::restore_target(hal)?;
    profile.restore_blocked_reason = energy::restore_issue(
        profile.restore.as_ref().and_then(|target| target.energy),
        profile.energy,
    );
    profile.can_restore = profile.restore.is_some() && profile.restore_blocked_reason.is_none();
    profile.blocked_reason = profile.blocked_reason.or_else(|| {
        crate::fanctl::dust::ensure_manual_allowed(hal)
            .err()
            .map(|e| format!("{e:#}"))
    });
    profile.software_active = crate::fanctl::software::read(hal)?.is_some();
    Ok(profile)
}

fn write(
    io: &impl ModeIo,
    snapshot: CoreSnapshot,
    permit: &mut dyn FnMut() -> Result<()>,
    wrote: &mut bool,
) -> Result<()> {
    permit()?;
    if let Some(mode) = snapshot.ac_mode {
        if byte(io, 1, 16)? != mode {
            *wrote = true;
            settings::apply(io, 1, &[Patch::new(16, [mode])], None).map_err(|e| anyhow!(e))?;
            permit()?;
        }
    }
    *wrote = true;
    io.send_mode(snapshot.command)?;
    permit()?;
    settings::apply(io, 1, &[Patch::new(1, [snapshot.mode])], None).map_err(|e| anyhow!(e))?;
    Ok(())
}

fn recover(io: &impl ModeIo, before: CoreSnapshot) -> Result<()> {
    // 即时命令失败也继续恢复保存字段，保留两个独立错误。
    let turbo = settings::apply(io, 4, &[Patch::new(8, [before.preferences[0]])], None);
    let dtt = settings::apply(io, 1, &[Patch::new(32, [before.preferences[1]])], None);
    let ac = before
        .ac_mode
        .map(|mode| settings::apply(io, 1, &[Patch::new(16, [mode])], None))
        .transpose();
    let command = io.send_mode(before.command);
    let saved = settings::apply(io, 1, &[Patch::new(1, [before.mode])], None);
    let errors = [
        turbo.err(),
        dtt.err(),
        ac.err(),
        command.err().map(|e| format!("{e:#}")),
        saved.err(),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    if !errors.is_empty() {
        bail!(errors.join("；"));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
