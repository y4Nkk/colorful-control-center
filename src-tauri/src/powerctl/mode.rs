//! OEM FnKey.Features.SetPowerMode 协议；page1 是保存值，cmd121/sub25 是即时命令。
use crate::hal::{Hal, oem_dchu::Dchu};
use anyhow::{Result, anyhow, bail};
use ccc_core::settings::{self, Patch, SettingsAccess};
use serde::Serialize;
use std::sync::atomic::Ordering;
pub mod boost;
pub(crate) mod cycle;
mod energy;
pub(crate) mod links;
pub(crate) mod manual;
pub use energy::State as EnergyState;
mod policy;
pub(crate) mod profile;
pub(crate) mod selection;
pub(crate) mod supply;
mod support;
mod system;
mod workflow;
pub use policy::Target;
#[cfg(test)]
pub(crate) use workflow::checkpoint_for_profile;
pub use workflow::{Plan, Snapshot, restore_checkpoint};
#[cfg(test)]
pub use workflow::{apply_preview, checkpoint};

pub(crate) const MODE_LABELS: [&str; 5] = ["安静", "省电", "性能", "娱乐", "静音增强"];

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
    pub available_modes: Vec<u8>,
    pub targets: Vec<Target>,
    pub software_active: bool,
    pub blocked_reason: Option<String>,
}

pub(crate) trait ModeIo: SettingsAccess {
    fn feature_flags(&self, command: i32) -> Result<u32>;
    fn send_mode(&self, command: u32) -> Result<()>;
    fn fan_info(&self) -> Result<[u8; 256]>;
    fn table(&self, selector: u8) -> Result<[u8; 256]>;
    fn reviewed_entertainment_options(&self) -> bool {
        false
    }
    fn acknowledged_entertainment_command(&self) -> Option<u32> {
        None
    }
}

impl ModeIo for Dchu {
    fn reviewed_entertainment_options(&self) -> bool {
        #[cfg(test)]
        return self.test_entertainment_options.load(Ordering::Relaxed);
        #[cfg(not(test))]
        crate::hal::firmware::reviewed_dsdt()
    }
    fn acknowledged_entertainment_command(&self) -> Option<u32> {
        let command = self.acknowledged_mode.load(Ordering::Relaxed);
        (command & 0x3f == 3).then_some(command)
    }
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
        self.set_data_sub(121, 25, command)?;
        self.acknowledged_mode.store(command, Ordering::Relaxed);
        Ok(())
    }
}

fn byte(io: &impl SettingsAccess, page: u8, offset: usize) -> Result<u8> {
    let bytes = io.read(page, offset, 1).map_err(|e| anyhow!(e))?;
    bytes
        .first()
        .copied()
        .ok_or_else(|| anyhow!("模式字段为空"))
}

pub(crate) fn current_mode(io: &impl SettingsAccess) -> Result<u8> {
    byte(io, 1, 1)
}

fn read_option_preferences(io: &impl SettingsAccess) -> Result<[u8; 2]> {
    let preferences = [byte(io, 4, 8)?, byte(io, 1, 32)?];
    if preferences.iter().any(|value| *value > 1) {
        bail!("Turbo 风扇或动态调校 DTT 的保存值未知");
    }
    Ok(preferences)
}

fn capture(io: &impl ModeIo, remember_ac: bool) -> Result<CoreSnapshot> {
    let mode = current_mode(io)?;
    if mode > 4 {
        bail!("当前 OEM 模式 {mode} 不在已核对的 0–4 范围");
    }
    let preferences = read_option_preferences(io)?;
    let configured = boost::command_for_current_preferences(io, mode, preferences)?;
    let command = if mode == 3 {
        io.acknowledged_entertainment_command()
            .filter(|command| command & !configured == 0)
            .unwrap_or(3)
    } else {
        configured
    };
    Ok(CoreSnapshot {
        mode,
        command,
        preferences,
        ac_mode: remember_ac.then(|| byte(io, 1, 16)).transpose()?,
    })
}

fn mode_command(mode: u8, preferences: [u8; 2]) -> u32 {
    u32::from(if mode == 4 { 0 } else { mode })
        | if matches!(mode, 2 | 3) {
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

pub(crate) fn mode_choices(io: &impl ModeIo) -> Result<(Vec<u8>, Option<String>)> {
    let issue = support::issue(io)?;
    let modes = if issue.is_some() {
        Vec::new()
    } else {
        available(io)?
    };
    Ok((modes, issue))
}

pub(crate) fn feature_flags(io: &impl ModeIo, caps: &[u8], command: i32) -> Result<u32> {
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
        let (modes, support_issue) = mode_choices(io)?;
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
            current_mode: current_mode(io)?,
            ac_mode: byte(io, 1, 16)?,
            energy,
            available_modes: modes,
            software_active: false,
            blocked_reason: support_issue.or_else(|| {
                hal.ensure_mode_change_allowed()
                    .err()
                    .map(|e| format!("{e:#}"))
            }),
            targets,
        })
    })?;
    profile.blocked_reason = profile.blocked_reason.or_else(|| {
        crate::fanctl::ensure_manual_allowed(hal)
            .err()
            .map(|e| format!("{e:#}"))
    });
    profile.software_active = crate::fanctl::software::active(hal)?;
    Ok(profile)
}

fn write_core(
    hal: &Hal,
    target: &Target,
    permit: &mut dyn FnMut() -> Result<()>,
    wrote: &mut bool,
) -> Result<()> {
    if let Some(mode) = target.ac_mode {
        permit()?;
        hal.with_dchu(|io| {
            if byte(io, 1, 16)? != mode {
                *wrote = true;
                settings::apply(io, 1, &[Patch::new(16, [mode])], None).map_err(|e| anyhow!(e))?;
            }
            Ok(())
        })?;
    }
    permit()?;
    hal.with_dchu(|io| {
        boost::validate_entertainment_command(io, target.command())?;
        *wrote = true;
        io.send_mode(target.command())
    })?;
    permit()?;
    hal.with_dchu(|io| {
        settings::apply(io, 1, &[Patch::new(1, [target.mode])], None).map_err(|e| anyhow!(e))?;
        Ok(())
    })
}

fn recover(io: &impl ModeIo, before: CoreSnapshot) -> Result<()> {
    boost::validate_entertainment_command(io, before.command)?;
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
