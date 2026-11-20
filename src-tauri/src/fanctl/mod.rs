//! 固件风扇曲线与模式：字段回读、失败自动恢复、独立进程监护。
mod checkpoint;
pub mod dust;
mod profile;
pub mod rpm;
pub(crate) mod saved;
pub mod software;
use crate::{hal::Hal, watchdog};
use anyhow::{Result, anyhow, bail};
use ccc_core::{
    fan::{OemCurvePoints, build_curve_package, oem_duty_byte},
    settings::{self, Patch},
};
pub(crate) use checkpoint::StoredCheckpoint;
pub use checkpoint::{FanCheckpoint, checkpoint, restore_checkpoint};
pub(crate) use profile::supports_noiseless;
pub use profile::{CurveKind, FanProfile};

pub const FAN_MODE_AUTO: u8 = 0;
pub(crate) const FAN_MODE_FULL_SPEED: u8 = 1;
pub const FAN_MODE_CUSTOM: u8 = 6;
pub(crate) const FAN_MODE_NOISELESS: u8 = 8;
pub(crate) const NOISELESS_FAN_LABEL: &str = "寂静模式";

pub(crate) fn read_firmware_profile(hal: &Hal) -> Result<FanProfile> {
    hal.with_dchu(|d| {
        let capabilities = d.read_bios_capabilities(0, 256)?;
        let mut profile =
            profile::decode(&d.read_package(13)?, &d.read_settings(4, 0, 256)?, &capabilities)?;
        // WMI96 bit10=1 时原厂禁用偏移（PSF 1.0 读能力字、旧页头活读）；能力位只叠加
        // 禁用，不推翻 RPM 布局的不支持结论。读取失败不判死，保留布局默认并提示诊断。
        match crate::powerctl::mode::feature_flags(d, &capabilities[..18], 96) {
            Ok(flags) => profile.offset_supported &= flags & (1 << 10) == 0,
            Err(error) => profile
                .warnings
                .push(format!("风扇偏移能力未知：{error:#}")),
        }
        Ok(profile)
    })
}

pub fn read_profile(hal: &Hal) -> Result<FanProfile> {
    let mut profile = read_firmware_profile(hal)?;
    if profile.curve_kind == CurveKind::RpmTable && profile.custom_supported {
        profile.rpm_fans = rpm::read(hal)?;
    }
    profile.can_restore = hal.recovery()?.fan_mode.is_some();
    profile.safety_status = hal.safety_status();
    profile.software_supported = hal.reviewed_dsdt
        && profile.fan_count == 2
        && ["cpu", "gpu1"].iter().all(|id| {
            profile.fans.iter().any(|f| f.present && f.id == *id)
                || profile.rpm_fans.iter().any(|f| f.id == *id)
        });
    profile.software = software::read(hal)?;
    profile.blocked_reason = dust::ensure_manual_allowed(hal)
        .err()
        .map(|e| format!("{e:#}"));
    Ok(profile)
}

pub fn validate_mode(mode: u8) -> Result<()> {
    if ![0, 1, 2, 5, 6, 8, 9].contains(&mode) {
        bail!("不支持此风扇模式");
    }
    Ok(())
}

pub fn validate_curve(hal: &Hal, points: [Option<OemCurvePoints>; 3]) -> Result<()> {
    profile::prepare(&read_profile(hal)?, points).map(|_| ())
}

fn capture_baseline(hal: &Hal, current: &FanProfile, patches: &[Patch]) -> Result<()> {
    let rpm_pairs = if current.curve_kind == CurveKind::RpmTable {
        Some(rpm::checkpoint(hal)?.pairs)
    } else {
        None
    };
    let (before, mode) = hal.with_dchu(|d| {
        let fields = patches
            .iter()
            .map(|p| {
                d.read_settings(4, p.offset as i32, p.data.len())
                    .map(|bytes| Patch::new(p.offset, bytes))
            })
            .collect::<Result<Vec<_>>>()?;
        let mode = saved::effective_mode(
            d,
            current.current_mode,
            current.available_modes.contains(&FAN_MODE_NOISELESS),
        )?;
        Ok((fields, mode))
    })?;
    let mut state = hal.recovery()?;
    if state.rpm_fan_pairs.is_none() {
        state.rpm_fan_pairs = rpm_pairs;
    }
    if state.fan_mode.is_none() {
        state.fan_mode = Some(mode);
    }
    if state.fan_package.is_none() {
        state.fan_package = profile::prepare(current, [None; 3])
            .ok()
            .map(|p| build_curve_package(&p));
    }
    settings::remember(&mut state.fan_page, before);
    Ok(())
}

fn fail_safe(hal: &Hal, result: Result<()>) -> Result<()> {
    if let Err(error) = result.and_then(|_| hal.pulse_fan_guard()) {
        let discarded = software::discard(hal);
        if matches!(discarded, Ok(true)) {
            hal.latch_software_fan_fault();
        }
        let restored = hal.with_dchu(|d| watchdog::restore_auto(d, hal.reviewed_dsdt));
        let finalized = if restored.is_ok() {
            hal.disarm_fan_guard().err()
        } else {
            if let Err(retry) = hal.request_fan_recovery() {
                log::error!("[watchdog] 无法安排补偿恢复：{retry:#}");
            }
            None
        };
        return Err(anyhow!(match restored {
            Ok(()) => format!(
                "{error:#}；已恢复风扇自动模式{}{}",
                discarded
                    .err()
                    .map_or(String::new(), |e| format!("；{e:#}")),
                finalized.map_or(String::new(), |e| format!("；恢复收尾失败：{e:#}"))
            ),
            Err(restore) => format!("{error:#}；自动恢复失败：{restore:#}"),
        }));
    }
    Ok(())
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
    dust::ensure_manual_allowed(hal)?;
    let current = read_profile(hal)?;
    let inputs = profile::prepare(&current, [cpu, gpu1, gpu2])?;
    let package = build_curve_package(&inputs);
    let mut patches = profile::page_patches(&inputs);
    patches.extend(hal.with_dchu(|io| saved::mode(io, FAN_MODE_CUSTOM))?);
    capture_baseline(hal, &current, &patches)?;
    software::stop_safely(hal)?;
    let result = hal.with_dchu(|d| {
        d.set_package(14, &package)?;
        settings::apply(d, 4, &patches, None).map_err(|e| anyhow!(e))?;
        d.set_data_sub(121, 1, FAN_MODE_CUSTOM as u32)
    });
    fail_safe(hal, result)
}

pub(crate) fn automatic_mode(hal: &Hal) -> Result<u8> {
    hal.with_dchu(|d| {
        let caps = d.read_bios_capabilities(0, 22)?;
        let quiet = supports_noiseless(&caps) && !crate::powerctl::mode::boost::turbo_enabled(d)?;
        saved::effective_mode(d, FAN_MODE_AUTO, quiet)
    })
}

pub fn set_mode(hal: &Hal, mode: u8) -> Result<u8> {
    let effective = if mode == FAN_MODE_AUTO {
        automatic_mode(hal)?
    } else {
        mode
    };
    apply_mode_policy(hal, mode, effective)?;
    Ok(effective)
}

pub fn apply_mode_policy(hal: &Hal, saved_mode: u8, mode: u8) -> Result<()> {
    dust::ensure_manual_allowed(hal)?;
    validate_mode(mode)?;
    validate_mode(saved_mode)?;
    if saved_mode != mode && (saved_mode != FAN_MODE_AUTO || mode != FAN_MODE_NOISELESS) {
        bail!("风扇保存策略与即时模式不一致");
    }
    let current = read_profile(hal)?;
    if !current.available_modes.contains(&mode) {
        bail!("固件未声明支持此风扇模式");
    }
    if mode == FAN_MODE_NOISELESS {
        crate::powerctl::mode::boost::ensure_noiseless_allowed(hal)?;
    }
    if mode == FAN_MODE_CUSTOM {
        if current.curve_kind == CurveKind::RpmTable {
            let wanted = current
                .rpm_fans
                .iter()
                .map(|fan| (fan.id.clone(), fan.points))
                .collect();
            return rpm::apply(hal, &wanted);
        }
        profile::prepare(&current, [None; 3])?;
    }
    let patches = hal.with_dchu(|io| saved::mode(io, saved_mode))?;
    capture_baseline(hal, &current, &patches)?;
    software::stop_safely(hal)?;
    let result = hal.with_dchu(|d| {
        d.set_data_sub(121, 1, mode as u32)?;
        settings::apply(d, 4, &patches, None).map_err(|e| anyhow!(e))?;
        Ok(())
    });
    fail_safe(hal, result)
}

pub fn set_offset(hal: &Hal, pct: u32) -> Result<()> {
    dust::ensure_manual_allowed(hal)?;
    if pct > 100 {
        bail!("风扇偏移范围 0–100%");
    }
    let current = read_profile(hal)?;
    let patches = hal.with_dchu(|io| saved::offset(io, pct as u8))?;
    capture_baseline(hal, &current, &patches)?;
    software::stop_safely(hal)?;
    fail_safe(
        hal,
        hal.with_dchu(|d| {
            d.set_data_sub(121, 14, oem_duty_byte(pct as f64) as u32)?;
            settings::apply(d, 4, &patches, None).map_err(|e| anyhow!(e))?;
            Ok(())
        }),
    )
}

pub fn restore_initial(hal: &Hal) -> Result<()> {
    dust::ensure_manual_allowed(hal)?;
    let (patches, package, rpm_pairs, mode) = {
        let state = hal.recovery()?;
        (
            state.fan_page.clone(),
            state.fan_package,
            state.rpm_fan_pairs.clone(),
            state.fan_mode,
        )
    };
    let Some(mode) = mode else {
        return Ok(());
    };
    validate_mode(mode)?;
    if mode == FAN_MODE_NOISELESS {
        crate::powerctl::mode::boost::ensure_noiseless_allowed(hal)?;
    }
    software::stop_safely(hal)?;
    fail_safe(
        hal,
        hal.with_dchu(|d| {
            if let Some(pairs) = &rpm_pairs {
                rpm::restore_pairs(d, pairs)?;
            }
            if let Some(package) = package {
                d.set_package(14, &package)?;
            } else if mode == FAN_MODE_CUSTOM && rpm_pairs.is_none() {
                bail!("初始曲线布局无效，请恢复固件默认");
            }
            if let Some(offset) = patches
                .iter()
                .find(|p| p.offset == 7)
                .and_then(|p| p.data.first())
            {
                d.set_data_sub(121, 14, oem_duty_byte(*offset as f64) as u32)?;
            }
            if !patches.is_empty() {
                settings::apply(d, 4, &patches, None).map_err(|e| anyhow!(e))?;
            }
            d.set_data_sub(121, 1, mode as u32)
        }),
    )?;
    let mut state = hal.recovery()?;
    state.fan_page.clear();
    state.fan_package = None;
    state.fan_mode = None;
    state.rpm_fan_pairs = None;
    Ok(())
}

pub fn restore_default(hal: &Hal) -> Result<()> {
    dust::ensure_manual_allowed(hal)?;
    let current = read_profile(hal)?;
    if current.curve_kind == CurveKind::RpmTable {
        return rpm::restore_default(hal);
    }
    let before = checkpoint(hal, true)?;
    let mut fields = profile::page_patches(&profile::prepare(&current, [None; 3])?);
    fields.extend(hal.with_dchu(|io| saved::mode(io, 0))?);
    capture_baseline(hal, &current, &fields)?;
    let result = reset_two_point(hal);
    if let Err(error) = result {
        let restored = restore_checkpoint(hal, &before).map_err(|e| format!("{e:#}"));
        return fail_safe(
            hal,
            Err(anyhow!(ccc_core::msr::with_recovery(
                format!("{error:#}"),
                restored
            ))),
        );
    }
    Ok(())
}

fn reset_two_point(hal: &Hal) -> Result<()> {
    software::stop_safely(hal)?;
    hal.with_dchu(|io| {
        io.set_data_sub(121, 34, 1)?;
        let defaults = profile::decode(
            &io.read_package(13)?,
            &[0; 256],
            &io.read_bios_capabilities(0, 256)?,
        )?;
        let mut fields = profile::page_patches(&profile::prepare(&defaults, [None; 3])?);
        fields.extend(saved::mode(io, 0)?);
        settings::apply(io, 4, &fields, None).map_err(|e| anyhow!(e))?;
        watchdog::restore_auto(io, hal.reviewed_dsdt)
    })
}
