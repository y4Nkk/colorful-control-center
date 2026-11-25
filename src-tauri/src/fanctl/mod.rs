//! 固件风扇事务、即时状态及独立进程监护。
mod checkpoint;
pub mod dust;
mod profile;
mod policy;
pub mod rpm;
pub(crate) mod saved;
pub mod software;

use crate::{
    hal::Hal,
    powerctl::mode::boost::{self, FanTarget},
};
use anyhow::{Result, bail, ensure};
use ccc_core::{
    fan::{OemCurvePoints, oem_duty_byte, oem_duty_percent, oem_fan_mode_bits},
    settings,
};
use sha2::{Digest, Sha256};
pub(crate) use checkpoint::StoredCheckpoint;
pub use checkpoint::{FanCheckpoint, checkpoint, restore_checkpoint};
pub(crate) use policy::ModePolicy;
pub use profile::{CurveKind, FanProfile};
pub(crate) use profile::{OffsetSource, supports_noiseless};

pub const FAN_MODE_AUTO: u8 = 0;
pub(crate) const FAN_MODE_FULL_SPEED: u8 = 1;
pub(crate) const FAN_MODE_MAXQ: u8 = 5;
pub const FAN_MODE_CUSTOM: u8 = 6;
pub(crate) const FAN_MODE_NOISELESS: u8 = 8;
pub(crate) const NOISELESS_FAN_LABEL: &str = "寂静模式";

pub(crate) fn ensure_manual_allowed(hal: &Hal) -> Result<()> {
    ensure!(
        !hal.fan_recovery_pending()
            && (!hal.software_fan_release_unverified()? || software::read(hal)?.is_some()),
        "尚未确认风扇已退出软件调速，暂不能修改风扇设置"
    );
    dust::ensure_manual_allowed(hal)
}

pub(crate) fn offset_supported(
    io: &impl crate::powerctl::mode::ModeIo,
    caps: &[u8],
) -> Result<bool> {
    if rpm::supported(caps) {
        return Ok(false);
    }
    crate::powerctl::mode::feature_flags(io, caps, 96).map(|flags| flags & (1 << 10) == 0)
}

pub(crate) fn read_firmware_profile(hal: &Hal) -> Result<FanProfile> {
    hal.with_dchu(|io| {
        let caps = io.read_bios_capabilities(0, 256)?;
        let mut profile = profile::decode(&io.read_fan_info()?, &caps)?;
        if profile.custom_supported
            && profile.curve_kind == CurveKind::TwoPoint
            && !hal.reviewed_dsdt
        {
            profile.custom_blocked_reason = Some("当前固件未确认两点曲线的原始回读通道。".into());
        }
        if saved::keeps_mode(&caps) {
            let mode = crate::powerctl::mode::current_mode(io)?;
            profile.offset_slot = Some(saved::mode_slot(mode)?);
        }
        profile.offset_blocked_reason = match offset_supported(io, &caps) {
            Ok(true) => match io.read_fan_offset(hal.reviewed_dsdt) {
                Ok(raw) => {
                    profile.offset_raw = Some(raw);
                    profile.offset_pct = Some(oem_duty_percent(raw));
                    None
                }
                Err(error) => Some(format!("无法确认 EC 即时风扇偏移：{error:#}")),
            },
            Ok(false) => Some("此机型不支持风扇加速偏移。".into()),
            Err(error) => Some(format!("风扇偏移能力未知：{error:#}")),
        };
        Ok(profile)
    })
}

pub fn read_profile(hal: &Hal) -> Result<FanProfile> {
    let mut profile = read_firmware_profile(hal)?;
    if profile.curve_kind == CurveKind::RpmTable && profile.custom_supported {
        profile.rpm_fans = rpm::read(hal)?;
    }
    profile.can_restore = hal.recovery()?.fan.is_some();
    profile.safety_status = hal.safety_status();
    profile.software_supported = hal.software_fan_supported()
        && profile.fan_count == 2
        && ["cpu", "gpu1"].iter().all(|id| {
            profile.fans.iter().any(|f| f.present && f.id == *id)
                || profile.rpm_fans.iter().any(|f| f.id == *id)
        });
    profile.software = software::read(hal)?;
    profile.blocked_reason = ensure_manual_allowed(hal).err().map(|e| format!("{e:#}"));
    if profile.custom_supported && profile.custom_blocked_reason.is_none() {
        profile.custom_blocked_reason =
            boost::ensure_fan_allowed(hal, FanTarget::Firmware(FAN_MODE_CUSTOM))
                .err()
                .map(|error| format!("{error:#}"));
    }
    if profile.offset_blocked_reason.is_none() {
        profile.offset_blocked_reason = if let Some(reason) = &profile.blocked_reason {
            Some(reason.clone())
        } else if profile.software.is_some() {
            Some("软件接管期间无法调节加速偏移。".into())
        } else if !matches!(profile.current_mode, FAN_MODE_AUTO | FAN_MODE_NOISELESS)
            || !profile.available_modes.contains(&profile.current_mode)
        {
            Some("仅自动或寂静模式可调节加速偏移。".into())
        } else {
            None
        };
    }
    Ok(profile)
}

pub fn validate_mode(mode: u8) -> Result<()> {
    ensure!(oem_fan_mode_bits(mode).is_some(), "不支持此风扇模式");
    Ok(())
}

pub fn validate_curve(hal: &Hal, points: [Option<OemCurvePoints>; 3]) -> Result<()> {
    let current = read_firmware_profile(hal)?;
    profile::prepare(&current, points)?;
    hal.with_dchu(|io| checkpoint::read_two_point(io, hal.reviewed_dsdt).map(|_| ()))
}

pub(crate) fn validate_custom_table(hal: &Hal, profile: &FanProfile) -> Result<()> {
    ensure!(profile.custom_supported, "固件未声明支持自定义风扇表");
    match profile.curve_kind {
        CurveKind::TwoPoint => {
            hal.with_dchu(|io| checkpoint::read_two_point(io, hal.reviewed_dsdt).map(|_| ()))
        }
        CurveKind::RpmTable => rpm::checkpoint(hal).map(|_| ()),
    }
}

pub(crate) fn release_on_exit(hal: &Hal) -> Result<()> {
    hal.close_software_fan_session()?;
    let _gate = hal.begin_write()?;
    software::stop_safely(hal)
}

pub fn apply_curve(
    hal: &Hal,
    cpu: Option<OemCurvePoints>,
    gpu1: Option<OemCurvePoints>,
    gpu2: Option<OemCurvePoints>,
) -> Result<()> {
    ensure_manual_allowed(hal)?;
    boost::ensure_fan_allowed(hal, FanTarget::Firmware(FAN_MODE_CUSTOM))?;
    let current = read_profile(hal)?;
    let updates = profile::prepare(&current, [cpu, gpu1, gpu2])?;
    ensure!(
        updates.iter().any(Option::is_some),
        "没有提供要写入的风扇曲线"
    );
    let before = checkpoint::capture(hal, &current, true)?;
    let package = profile::edit_package(before.two_point_package()?, &updates);
    let mut patches = profile::page_patches(&updates);
    patches.extend(hal.with_dchu(|io| saved::mode(io, FAN_MODE_CUSTOM))?);
    checkpoint::transact(hal, &before, |io| {
        checkpoint::write_two_point(io, hal.reviewed_dsdt, &package)?;
        settings::apply(io, 4, &patches, None).map_err(anyhow::Error::msg)?;
        io.set_fan_mode_verified(FAN_MODE_CUSTOM)
    })
}

pub(crate) fn automatic_mode(hal: &Hal) -> Result<u8> {
    hal.with_dchu(|d| {
        let caps = d.read_bios_capabilities(0, 22)?;
        if !supports_noiseless(&caps) {
            return Ok(FAN_MODE_AUTO);
        }
        saved::automatic_target(d, crate::powerctl::mode::current_mode(d)?)
    })
}

pub fn set_mode(hal: &Hal, mode: u8) -> Result<u8> {
    let effective = if mode == FAN_MODE_AUTO {
        automatic_mode(hal)?
    } else {
        mode
    };
    let saved = if mode == FAN_MODE_NOISELESS
        && hal.with_dchu(|io| crate::powerctl::mode::current_mode(io))? == 3
    {
        FAN_MODE_AUTO
    } else {
        mode
    };
    apply_mode_policy(hal, ModePolicy::explicit(effective, saved, None))?;
    Ok(effective)
}

pub(crate) fn apply_mode_policy(hal: &Hal, policy: ModePolicy) -> Result<()> {
    ensure_manual_allowed(hal)?;
    policy.validate()?;
    let mode = policy.mode();
    let current = read_profile(hal)?;
    boost::ensure_fan_allowed(hal, FanTarget::Firmware(mode.unwrap_or(current.current_mode)))?;
    if let Some(mode) = mode {
        ensure!(
            current.available_modes.contains(&mode),
            "固件未声明支持此风扇模式"
        );
        if mode == FAN_MODE_CUSTOM {
            validate_custom_table(hal, &current)?;
        }
    }
    let before = checkpoint::capture(hal, &current, false)?;
    let patches = hal.with_dchu(|io| saved::mode(io, policy.saved_mode()))?;
    checkpoint::transact(hal, &before, |io| {
        if let Some(mode) = mode {
            io.set_fan_mode_verified(mode)?;
        }
        settings::apply(io, 4, &patches, None).map_err(anyhow::Error::msg)?;
        Ok(())
    })
}

pub(crate) fn handoff_for_turbo(hal: &Hal, attempted: &mut bool) -> Result<()> {
    ensure_manual_allowed(hal)?;
    hal.with_dchu(|io| {
        if matches!(io.read_fan_mode()?, FAN_MODE_CUSTOM | FAN_MODE_NOISELESS) {
            // 先交接即时档位，目标预设随后提交自己的记忆槽。
            *attempted = true;
            io.set_fan_mode_verified(FAN_MODE_AUTO)?;
        }
        Ok(())
    })
}

pub(crate) fn set_manual_offset(hal: &Hal, expected: OffsetSource, pct: u32) -> Result<()> {
    let current = read_profile(hal)?;
    if let Some(reason) = &current.offset_blocked_reason {
        bail!("{reason}");
    }
    ensure!(
        current.current_mode == expected.mode,
        "风扇档位已变化，请重新调整"
    );
    ensure!(
        current.offset_slot == expected.slot,
        "风扇偏移记忆槽已变化，请重新调整"
    );
    write_offset(hal, &current, pct)
}

pub(crate) fn set_mode_offset(hal: &Hal, pct: u32) -> Result<()> {
    ensure_manual_allowed(hal)?;
    write_offset(hal, &read_profile(hal)?, pct)
}

fn write_offset(hal: &Hal, current: &FanProfile, pct: u32) -> Result<()> {
    ensure!(pct <= 100, "风扇偏移范围 0–100%");
    ensure!(current.offset_raw.is_some(), "无法确认写入前的 EC 风扇偏移");
    let before = checkpoint::capture(hal, current, false)?;
    let patches = saved::offset(current.offset_slot, pct as u8);
    let raw = oem_duty_byte(f64::from(pct));
    checkpoint::transact(hal, &before, |io| {
        io.set_fan_offset_verified(hal.reviewed_dsdt, raw)?;
        settings::apply(io, 4, &patches, None).map_err(anyhow::Error::msg)?;
        Ok(())
    })
}

fn restore_initial(hal: &Hal) -> Result<()> {
    ensure_manual_allowed(hal)?;
    let Some(initial) = hal.recovery()?.fan.clone() else {
        bail!("尚无初始风扇恢复记录，请重新读取");
    };
    boost::ensure_fan_allowed(hal, FanTarget::Firmware(initial.mode()))?;
    let before = checkpoint(hal, initial.has_tables())?;
    initial.check_restorable_from(&before)?;
    checkpoint::with_rollback(hal, &before, restore_checkpoint(hal, &initial))?;
    hal.recovery()?.fan = None;
    Ok(())
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum RestoreTarget {
    Initial,
    FirmwareDefault,
    InitialAndFirmwareDefault,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RestoreView {
    #[serde(flatten)]
    profile: FanProfile,
    pub(crate) restore_revision: String,
}

pub(crate) fn read_restore_view(hal: &Hal) -> Result<RestoreView> {
    let profile = read_profile(hal)?;
    let initial = hal.recovery()?.fan.as_ref().map(FanCheckpoint::stored);
    let state = serde_json::to_vec(&(&profile, profile.offset_raw, initial))?;
    Ok(RestoreView {
        profile,
        restore_revision: format!("{:x}", Sha256::digest(state)),
    })
}

pub(crate) fn restore(hal: &Hal, target: RestoreTarget, expected_revision: &str) -> Result<()> {
    ensure_manual_allowed(hal)?;
    ensure!(
        read_restore_view(hal)?.restore_revision == expected_revision,
        "风扇恢复目标已变化，请重新查看后确认"
    );
    match target {
        RestoreTarget::Initial => restore_initial(hal),
        RestoreTarget::FirmwareDefault => restore_default(hal),
        RestoreTarget::InitialAndFirmwareDefault => {
            restore_initial(hal)
                .map_err(|error| anyhow::anyhow!("初始风扇设置恢复失败：{error:#}"))?;
            restore_default(hal)
                .map_err(|error| anyhow::anyhow!("初始风扇设置已恢复，固件默认设置恢复失败：{error:#}"))
        }
    }
}

fn restore_default(hal: &Hal) -> Result<()> {
    ensure_manual_allowed(hal)?;
    let current = read_profile(hal)?;
    if current.curve_kind == CurveKind::RpmTable {
        return rpm::restore_default(hal);
    }
    let before = checkpoint::capture(hal, &current, true)?;
    before.two_point_package()?;
    let (caps, mut patches, offset) = hal.with_dchu(|io| {
        let caps = io.read_bios_capabilities(0, 256)?;
        let offset = offset_supported(io, &caps)?;
        ensure!(
            !offset || current.offset_raw.is_some(),
            "无法确认写入前的 EC 风扇偏移"
        );
        Ok((caps, saved::mode(io, FAN_MODE_AUTO)?, offset))
    })?;
    checkpoint::transact(hal, &before, |io| {
        io.set_data_sub(121, 34, 1)?;
        let info = io.read_fan_info()?;
        let defaults = profile::decode(&info, &caps)?;
        let package = checkpoint::read_two_point_from(io, hal.reviewed_dsdt, &info)?;
        patches.extend(profile::live_page_patches(&defaults, &package));
        if offset {
            io.set_fan_offset_verified(hal.reviewed_dsdt, 0)?;
            patches.extend(saved::offset(current.offset_slot, 0));
        }
        io.set_fan_mode_verified(FAN_MODE_AUTO)?;
        settings::apply(io, 4, &patches, None).map_err(anyhow::Error::msg)?;
        Ok(())
    })
}
