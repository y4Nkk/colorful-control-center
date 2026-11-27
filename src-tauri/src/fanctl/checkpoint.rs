use super::{CurveKind, FanProfile, saved, software};
use crate::{
    hal::{Hal, oem_dchu::Dchu},
    powerctl::mode::boost::{self, FanTarget},
};
use anyhow::{Result, anyhow, bail, ensure};
use ccc_core::{
    fan::{oem_duty_percent, oem_page4_offsets},
    msr::with_recovery,
    settings::{self, Patch},
};
mod storage;
pub(crate) use storage::StoredCheckpoint;

#[derive(Clone, PartialEq)]
pub struct FanCheckpoint {
    software: Option<ccc_core::software_fan::Curves>,
    package: Option<[u8; 256]>,
    rpm_pairs: Option<Vec<ccc_core::rpm::Pair>>,
    fields: Vec<Patch>,
    mode: u8,
    offset_raw: Option<u8>,
}

impl FanCheckpoint {
    pub(crate) fn software_curves(&self) -> Option<&ccc_core::software_fan::Curves> {
        self.software.as_ref()
    }
    pub(crate) fn mode(&self) -> u8 {
        self.mode
    }
    pub(crate) fn offset(&self) -> Option<u8> {
        self.offset_raw.map(oem_duty_percent)
    }
    pub(crate) fn entertainment_quiet(&self) -> Option<bool> {
        self.fields
            .iter()
            .find(|patch| patch.offset == saved::ENTERTAINMENT_QUIET_OFFSET)
            .map(|patch| patch.data == [1])
    }
    pub(crate) fn remember_entertainment_quiet(&mut self, enabled: bool) {
        settings::remember(
            &mut self.fields,
            vec![Patch::new(
                saved::ENTERTAINMENT_QUIET_OFFSET,
                [u8::from(enabled)],
            )],
        );
    }
    pub(crate) fn saved_settings_differ(&self, other: &Self) -> bool {
        self.package != other.package
            || self.rpm_pairs != other.rpm_pairs
            || self
                .fields
                .iter()
                .filter(|patch| patch.offset >= 16)
                .ne(other.fields.iter().filter(|patch| patch.offset >= 16))
    }
    pub(super) fn two_point_package(&self) -> Result<[u8; 256]> {
        self.package
            .ok_or_else(|| anyhow!("缺少已读取的即时两点曲线"))
    }
    pub(super) fn has_tables(&self) -> bool {
        self.package.is_some() || self.rpm_pairs.is_some()
    }
    pub(super) fn with_rpm_tables(mut self, pairs: Vec<ccc_core::rpm::Pair>) -> Self {
        self.rpm_pairs = Some(pairs);
        self
    }
    pub(super) fn check_restorable_from(&self, current: &Self) -> Result<()> {
        ensure!(
            (self.package.is_none() || current.package.is_some())
                && (self.rpm_pairs.is_none() || current.rpm_pairs.is_some())
                && (self.offset_raw.is_none() || current.offset_raw.is_some()),
            "缺少恢复前的即时曲线或偏移，请重新读取设备状态"
        );
        Ok(())
    }
}

pub fn checkpoint(hal: &Hal, include_curve: bool) -> Result<FanCheckpoint> {
    capture(hal, &super::read_profile(hal)?, include_curve)
}

pub(super) fn capture(
    hal: &Hal,
    current: &FanProfile,
    include_curve: bool,
) -> Result<FanCheckpoint> {
    let custom = include_curve && current.custom_supported;
    let rpm_pairs = (custom && current.curve_kind == CurveKind::RpmTable)
        .then(|| super::rpm::checkpoint(hal).map(|snapshot| snapshot.pairs))
        .transpose()?;
    let (package, fields, offset_raw) = hal.with_dchu(|io| {
        ensure!(
            io.read_fan_mode()? == current.current_mode,
            "EC 风扇档位已变化，请重新读取"
        );
        let package = (custom && current.curve_kind == CurveKind::TwoPoint)
            .then(|| read_two_point(io, hal.reviewed_dsdt))
            .transpose()?;
        let page = io.read_settings(4, 0, 256)?;
        let mut fields = [saved::MODE_OFFSET, saved::FAN_OFFSET]
            .map(|offset| Patch::new(offset, [page[offset]]))
            .to_vec();
        if package.is_some() {
            for (index, _) in current
                .fans
                .iter()
                .enumerate()
                .filter(|(_, fan)| fan.present)
            {
                let (duty, temp, slope) = oem_page4_offsets(index);
                fields.extend([
                    Patch::new(duty, page[duty..duty + 4].to_vec()),
                    Patch::new(temp, page[temp..temp + 4].to_vec()),
                    Patch::new(slope, page[slope..slope + 6].to_vec()),
                ]);
            }
        }
        fields.extend(saved::capture_slots(io)?);
        let offset_raw = current
            .offset_raw
            .map(|_| io.read_fan_offset(hal.reviewed_dsdt))
            .transpose()?;
        Ok((package, fields, offset_raw))
    })?;
    Ok(FanCheckpoint {
        software: current.software.clone(),
        package,
        rpm_pairs,
        fields,
        mode: current.current_mode,
        offset_raw,
    })
}

pub(super) fn read_two_point(io: &Dchu, reviewed_dsdt: bool) -> Result<[u8; 256]> {
    let info = io.read_fan_info()?;
    read_two_point_from(io, reviewed_dsdt, &info)
}

pub(super) fn read_two_point_from(
    io: &Dchu,
    reviewed_dsdt: bool,
    info: &[u8; 256],
) -> Result<[u8; 256]> {
    let bitmap = ccc_core::fan_sensors::fan_channels(info)
        .filter(|bits| bits & 8 == 0)
        .ok_or_else(|| anyhow!("EC 两点曲线通道不明"))?;
    io.read_fan_curve_package(reviewed_dsdt, info, bitmap)
}

pub(super) fn write_two_point(io: &Dchu, reviewed_dsdt: bool, package: &[u8; 256]) -> Result<()> {
    ensure!(reviewed_dsdt, "当前固件未确认 EC 两点曲线读写通道");
    io.set_package(14, package)?;
    ensure!(
        read_two_point(io, reviewed_dsdt)? == *package,
        "EC 两点曲线回读与目标不一致"
    );
    Ok(())
}

pub(super) fn edit_two_point(
    hal: &Hal,
    before: &FanCheckpoint,
    package: &[u8; 256],
    patches: &[Patch],
) -> Result<()> {
    let original = before.two_point_package()?;
    remember_baseline(hal, before)?;
    hal.with_dchu(|io| {
        super::require_custom_mode(io.read_fan_mode()?)?;
        let result = (|| -> Result<()> {
            write_two_point(io, hal.reviewed_dsdt, package)?;
            super::require_custom_mode(io.read_fan_mode()?)?;
            settings::apply(io, 4, patches, None).map_err(anyhow::Error::msg)?;
            Ok(())
        })();
        if let Err(error) = result {
            let recovery = write_two_point(io, hal.reviewed_dsdt, &original)
                .map_err(|error| format!("恢复即时曲线：{error:#}"));
            bail!(with_recovery(format!("{error:#}"), recovery));
        }
        Ok(())
    })
}

pub(super) fn remember_baseline(hal: &Hal, before: &FanCheckpoint) -> Result<()> {
    let mut state = hal.recovery()?;
    if let Some(initial) = &mut state.fan {
        initial.package = initial.package.or(before.package);
        if initial.rpm_pairs.is_none() {
            initial.rpm_pairs.clone_from(&before.rpm_pairs);
        }
        initial.offset_raw = initial.offset_raw.or(before.offset_raw);
        settings::remember(&mut initial.fields, before.fields.clone());
    } else {
        let mut initial = before.clone();
        initial.software = None;
        state.fan = Some(initial);
    }
    Ok(())
}

pub(super) fn remember_preset_baseline(hal: &Hal, before: &FanCheckpoint) -> Result<()> {
    let fields = before
        .fields
        .iter()
        .filter(|field| {
            matches!(
                field.offset,
                saved::MODE_OFFSET
                    | saved::FAN_OFFSET
                    | saved::ENTERTAINMENT_QUIET_OFFSET
                    | saved::MODE_SLOTS_OFFSET
            )
        })
        .cloned()
        .collect();
    remember_baseline(
        hal,
        &FanCheckpoint {
            software: None,
            package: None,
            rpm_pairs: None,
            fields,
            mode: before.mode,
            offset_raw: before.offset_raw,
        },
    )
}

pub(super) fn transact(
    hal: &Hal,
    before: &FanCheckpoint,
    attempt: impl FnOnce(&Dchu) -> Result<()>,
) -> Result<()> {
    software::stop_safely(hal)?;
    remember_baseline(hal, before)?;
    with_rollback(hal, before, hal.with_dchu(attempt))
}

pub(super) fn with_rollback(hal: &Hal, before: &FanCheckpoint, result: Result<()>) -> Result<()> {
    if let Err(error) = result {
        let recovery = restore_checkpoint(hal, before).map_err(|error| format!("{error:#}"));
        bail!(with_recovery(format!("{error:#}"), recovery));
    }
    Ok(())
}

pub fn restore_checkpoint(hal: &Hal, before: &FanCheckpoint) -> Result<()> {
    super::ensure_manual_allowed(hal)?;
    software::stop_safely(hal)?;
    let errors = hal.with_dchu(|io| Ok(restore_hardware(hal, io, before)))?;
    if !errors.is_empty() {
        bail!(errors.join("；"));
    }
    if let Some(curves) = &before.software {
        software::apply(hal, curves)?;
    }
    Ok(())
}

fn restore_hardware(hal: &Hal, io: &Dchu, before: &FanCheckpoint) -> Vec<String> {
    let mut errors = Vec::new();
    let mut record = |label: &str, result: Result<()>| {
        if let Err(error) = result {
            errors.push(format!("{label}：{error:#}"));
        }
    };
    if let Some(pairs) = &before.rpm_pairs {
        record("十级 RPM 表", super::rpm::restore_pairs(io, pairs));
    }
    if let Some(package) = &before.package {
        record("两点曲线", write_two_point(io, hal.reviewed_dsdt, package));
    }
    for field in &before.fields {
        let current = io.read_settings(4, field.offset as i32, field.data.len());
        if current.is_ok_and(|data| data == field.data) {
            continue;
        }
        record(
            &format!("保存字段 {}", field.offset),
            settings::write_verified(io, 4, std::slice::from_ref(field), None)
                .map_err(anyhow::Error::msg),
        );
    }
    if let Some(offset) = before.offset_raw {
        if !io
            .read_fan_offset(hal.reviewed_dsdt)
            .is_ok_and(|raw| raw == offset)
        {
            record(
                "即时偏移",
                io.set_fan_offset_verified(hal.reviewed_dsdt, offset),
            );
        }
    }
    let mode = boost::ensure_fan_allowed_on(io, FanTarget::Firmware(before.mode))
        .and_then(|()| io.restore_fan_mode(before.mode));
    record("即时档位", mode);
    errors
}
