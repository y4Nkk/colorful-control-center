use super::{FAN_MODE_NOISELESS, NOISELESS_FAN_LABEL, saved};
use crate::hal::Hal;
use anyhow::{Result, anyhow, ensure};
use ccc_core::{
    fan::{build_curve_package, oem_duty_byte},
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
    offset: u8,
}
impl FanCheckpoint {
    pub(crate) fn software_curves(&self) -> Option<&ccc_core::software_fan::Curves> {
        self.software.as_ref()
    }
    pub(crate) fn mode(&self) -> u8 {
        self.mode
    }
    pub(crate) fn offset(&self) -> u8 {
        self.offset
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
                .collect::<Vec<_>>()
                != other
                    .fields
                    .iter()
                    .filter(|patch| patch.offset >= 16)
                    .collect::<Vec<_>>()
    }
}

pub fn checkpoint(hal: &Hal, include_curve: bool) -> Result<FanCheckpoint> {
    let current = super::read_profile(hal)?;
    super::validate_mode(current.current_mode)?;
    let capture_curve = include_curve
        && (current.custom_supported || current.current_mode == super::FAN_MODE_CUSTOM);
    let rpm_layout = current.curve_kind == super::CurveKind::RpmTable;
    let inputs = if capture_curve && !rpm_layout {
        Some(super::profile::prepare(&current, [None; 3])?)
    } else {
        None
    };
    let rpm_pairs = if capture_curve && rpm_layout {
        Some(super::rpm::checkpoint(hal)?.pairs)
    } else {
        None
    };
    let mut layout = inputs
        .as_ref()
        .map(super::profile::page_patches)
        .unwrap_or_default();
    layout.push(Patch::new(5, [0]));
    layout.push(Patch::new(7, [0]));
    let (fields, mode) = hal.with_dchu(|d| {
        layout.extend(super::saved::capture_slots(d)?);
        let fields = layout
            .iter()
            .map(|p| {
                d.read_settings(4, p.offset as i32, p.data.len())
                    .map(|bytes| Patch::new(p.offset, bytes))
            })
            .collect::<Result<Vec<_>>>()?;
        let mode = saved::effective_mode(
            d,
            current.current_mode,
            current.available_modes.contains(&FAN_MODE_NOISELESS)
                && !crate::powerctl::mode::boost::turbo_enabled(d)?,
        )?;
        Ok((fields, mode))
    })?;
    Ok(FanCheckpoint {
        software: current.software,
        rpm_pairs,
        package: inputs.as_ref().map(build_curve_package),
        fields,
        mode,
        offset: current.offset_pct,
    })
}

pub fn restore_checkpoint(hal: &Hal, checkpoint: &FanCheckpoint) -> Result<()> {
    super::dust::ensure_manual_allowed(hal)?;
    if checkpoint.entertainment_quiet().is_some() {
        ensure!(
            hal.with_dchu(|d| Ok(super::supports_noiseless(&d.read_bios_capabilities(0, 22)?)))?,
            "固件未声明支持恢复娱乐模式的{NOISELESS_FAN_LABEL}"
        );
    }
    if checkpoint.mode == super::FAN_MODE_NOISELESS {
        crate::powerctl::mode::boost::ensure_noiseless_allowed(hal)?;
    }
    super::software::stop_safely(hal)?;
    let result = hal.with_dchu(|d| {
        if let Some(pairs) = &checkpoint.rpm_pairs {
            super::rpm::restore_pairs(d, pairs)?;
        }
        if let Some(package) = checkpoint.package {
            d.set_package(14, &package)?;
        }
        settings::apply(d, 4, &checkpoint.fields, None).map_err(|e| anyhow!(e))?;
        d.set_data_sub(121, 14, oem_duty_byte(checkpoint.offset as f64) as u32)?;
        d.set_data_sub(121, 1, checkpoint.mode as u32)
    });
    super::fail_safe(hal, result)?;
    if let Some(curves) = &checkpoint.software {
        super::fail_safe(hal, hal.ensure_software_fan_restart_allowed())?;
        super::software::apply(hal, curves)?;
    }
    Ok(())
}
