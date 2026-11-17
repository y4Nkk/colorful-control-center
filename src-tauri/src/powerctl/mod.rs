//! PL 与电压调节：字段事务、MSR 回读、首次写入前基线恢复。
pub mod mode;
pub mod runtime;
use crate::hal::Hal;
use anyhow::{Result, anyhow, bail};
use ccc_core::{
    msr,
    settings::{self, Patch},
};
use serde::{Deserialize, Serialize};

pub const PL_MIN_W: f64 = 35.0;
pub const PL_MAX_W: f64 = 157.0;
pub const UV_MIN_MV: i32 = -150;
pub const UV_MAX_MV: i32 = 50;

#[derive(Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct CpuControls {
    pub pl1_w: Option<f64>,
    pub pl2_w: Option<f64>,
    pub core_offset_mv: Option<i32>,
    pub gt_offset_mv: Option<i32>,
}
impl CpuControls {
    pub fn is_empty(&self) -> bool {
        self.pl1_w.is_none()
            && self.pl2_w.is_none()
            && self.core_offset_mv.is_none()
            && self.gt_offset_mv.is_none()
    }
}

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PowerValues {
    pub pl1_w: Option<f64>,
    pub pl2_w: Option<f64>,
    pub core_offset_mv: Option<f64>,
    pub gt_offset_mv: Option<f64>,
}

fn read_runtime(hal: &Hal) -> Result<(PowerValues, Vec<String>)> {
    let io = hal.open_msr()?;
    let unit = msr::power_unit(io.read_msr(msr::MSR_RAPL_POWER_UNIT)?);
    let (pl1, pl2) = msr::decode_power_limits(io.read_msr(msr::MSR_PKG_POWER_LIMIT)?, unit);
    let mut warnings = Vec::new();
    let mut voltage = |plane| match msr::read_voltage_units(&io, plane) {
        Ok(units) => Some(units as f64 / 1.024),
        Err(error) => {
            warnings.push(format!("电压域 {plane}：{error}"));
            None
        }
    };
    Ok((
        PowerValues {
            pl1_w: Some(pl1),
            pl2_w: Some(pl2),
            core_offset_mv: voltage(0),
            gt_offset_mv: voltage(1),
        },
        warnings,
    ))
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PowerProfile {
    pub pl1_w: Option<f64>,
    pub pl2_w: Option<f64>,
    pub pl_time_s: Option<f64>,
    pub core_offset_mv: Option<f64>,
    pub gt_offset_mv: Option<f64>,
    pub default_pl1_w: Option<f64>,
    pub default_pl2_w: Option<f64>,
    pub default_core_offset_mv: Option<f64>,
    pub msr_runtime_available: bool,
    pub msr_runtime_error: Option<String>,
    pub can_restore: bool,
    pub runtime: PowerValues,
    pub runtime_warnings: Vec<String>,
    pub warnings: Vec<String>,
}

fn i32_at(bytes: &[u8], at: usize) -> i32 {
    i32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}
fn finite(value: f64) -> Option<f64> {
    value.is_finite().then_some(value)
}
fn scale_w(raw: u32) -> Option<f64> {
    let value = if raw > 1_000_000 {
        raw as f64 / 1_000_000.0
    } else if raw > 1_000 {
        raw as f64 / 1_000.0
    } else {
        raw as f64
    };
    (value > 0.0 && value < 500.0).then_some(value)
}

fn saved_values(page: &[u8]) -> PowerValues {
    PowerValues {
        pl1_w: finite(i32_at(page, 33) as f64).filter(|v| *v > 0.0),
        pl2_w: finite(i32_at(page, 37) as f64).filter(|v| *v > 0.0),
        core_offset_mv: Some(i32_at(page, 61) as f64),
        gt_offset_mv: Some(i32_at(page, 69) as f64),
    }
}

pub fn read_profile(hal: &Hal) -> Result<PowerProfile> {
    let (page, defaults) =
        hal.with_dchu(|d| Ok((d.read_settings(6, 0, 256)?, d.read_package(18)?)))?;
    let (runtime, runtime_warnings, runtime_error) = match read_runtime(hal) {
        Ok((values, warnings)) => (values, warnings, None),
        Err(error) => (
            PowerValues::default(),
            Vec::new(),
            Some(format!("{error:#}")),
        ),
    };
    let raw = u16::from_le_bytes([defaults[176], defaults[177]]);
    let default_uv = if raw & 0x8000 != 0 {
        (raw ^ 0x8000) as f64
    } else {
        -(raw as f64)
    };
    let recovery = hal.recovery()?;
    let saved = saved_values(&page);
    Ok(PowerProfile {
        pl1_w: saved.pl1_w,
        pl2_w: saved.pl2_w,
        pl_time_s: finite(f32::from_le_bytes(page[41..45].try_into().unwrap()) as f64),
        core_offset_mv: saved.core_offset_mv,
        gt_offset_mv: saved.gt_offset_mv,
        default_pl1_w: scale_w(u32::from_le_bytes(defaults[8..12].try_into().unwrap())),
        default_pl2_w: scale_w(u32::from_le_bytes(defaults[12..16].try_into().unwrap())),
        default_core_offset_mv: Some(default_uv),
        msr_runtime_available: runtime_error.is_none(),
        msr_runtime_error: runtime_error,
        runtime,
        runtime_warnings,
        can_restore: !recovery.power.is_empty()
            || recovery.runtime_pl.is_some()
            || !recovery.voltage.is_empty(),
        warnings: vec!["保存值不等于实时生效值".into()],
    })
}

pub fn validate_pl(pl1: Option<f64>, pl2: Option<f64>) -> Result<()> {
    if pl1.is_none() && pl2.is_none() {
        bail!("没有提供功耗变更");
    }
    for value in [pl1, pl2].into_iter().flatten() {
        if !value.is_finite() || !(PL_MIN_W..=PL_MAX_W).contains(&value) || value.fract() != 0.0 {
            bail!("功耗须为 {PL_MIN_W}–{PL_MAX_W}W 的整数");
        }
    }
    validate_pl_order(pl1, pl2)?;
    Ok(())
}

pub fn validate_pl_order(pl1: Option<f64>, pl2: Option<f64>) -> Result<()> {
    if matches!((pl1, pl2), (Some(a), Some(b)) if !a.is_finite() || !b.is_finite() || a > b) {
        bail!("持续功耗上限不能高于短时功耗上限，且当前功耗读数须有效");
    }
    Ok(())
}

pub fn validate_voltage(core: Option<i32>, gt: Option<i32>) -> Result<()> {
    if core.is_none() && gt.is_none() {
        bail!("没有提供电压变更");
    }
    if core.is_some_and(|v| !(UV_MIN_MV..=UV_MAX_MV).contains(&v))
        || gt.is_some_and(|v| !(-150..=0).contains(&v))
    {
        bail!("核心电压偏移范围 −150…+50 mV，核显电压偏移范围 −150…0 mV");
    }
    Ok(())
}

pub fn power_patches(pl1: Option<f64>, pl2: Option<f64>) -> Result<Vec<Patch>> {
    validate_pl(pl1, pl2)?;
    Ok([(33, pl1), (37, pl2)]
        .into_iter()
        .filter_map(|(at, v)| v.map(|v| Patch::new(at, (v as i32).to_le_bytes())))
        .collect())
}

pub fn voltage_patches(core: Option<i32>, gt: Option<i32>) -> Result<Vec<Patch>> {
    validate_voltage(core, gt)?;
    Ok([(61, core), (69, gt)]
        .into_iter()
        .filter_map(|(at, v)| v.map(|v| Patch::new(at, v.to_le_bytes())))
        .collect())
}

pub fn apply_patches(hal: &Hal, patches: &[Patch]) -> Result<()> {
    let before = checkpoint_patches(hal, patches)?;
    settings::remember(&mut hal.recovery()?.power, before);
    hal.with_dchu(|d| {
        settings::apply(d, 6, patches, Some(&Patch::new(32, [1])))
            .map(|_| ())
            .map_err(|e| anyhow!(e))
    })
}

pub fn checkpoint_patches(hal: &Hal, patches: &[Patch]) -> Result<Vec<Patch>> {
    if patches.is_empty() {
        bail!("没有提供保存设置变更");
    }
    hal.with_dchu(|d| {
        let mut before = patches
            .iter()
            .map(|p| {
                d.read_settings(6, p.offset as i32, p.data.len())
                    .map(|data| Patch::new(p.offset, data))
            })
            .collect::<Result<Vec<_>>>()?;
        before.push(Patch::new(32, d.read_settings(6, 32, 1)?));
        Ok(before)
    })
}

pub fn restore_patches(hal: &Hal, before: &[Patch]) -> Result<()> {
    let trigger = before
        .iter()
        .find(|p| p.offset == 32)
        .ok_or_else(|| anyhow!("保存恢复快照缺少 IsSaved 字段"))?;
    let fields = before
        .iter()
        .filter(|p| p.offset != 32)
        .cloned()
        .collect::<Vec<_>>();
    hal.with_dchu(|d| {
        settings::apply(d, 6, &fields, Some(trigger))
            .map(|_| ())
            .map_err(|e| anyhow!(e))
    })
}

pub fn saved_patches(hal: &Hal, controls: CpuControls) -> Result<Vec<Patch>> {
    if controls.is_empty() {
        bail!("没有提供 CPU 变更");
    }
    let mut patches = Vec::new();
    if controls.pl1_w.is_some() || controls.pl2_w.is_some() {
        validate_pl(controls.pl1_w, controls.pl2_w)?;
        let saved = if controls.pl1_w.is_none() || controls.pl2_w.is_none() {
            saved_values(&hal.with_dchu(|d| d.read_settings(6, 0, 256))?)
        } else {
            PowerValues::default()
        };
        let first = controls.pl1_w.or(saved.pl1_w);
        let second = controls.pl2_w.or(saved.pl2_w);
        if first.is_none() || second.is_none() {
            bail!("当前保存功耗不完整，需同时设置持续功耗上限和短时功耗上限");
        }
        validate_pl_order(first, second)?;
        patches.extend(power_patches(controls.pl1_w, controls.pl2_w)?);
    }
    if controls.core_offset_mv.is_some() || controls.gt_offset_mv.is_some() {
        patches.extend(voltage_patches(
            controls.core_offset_mv,
            controls.gt_offset_mv,
        )?);
    }
    Ok(patches)
}

pub fn apply_saved(hal: &Hal, controls: CpuControls) -> Result<()> {
    apply_patches(hal, &saved_patches(hal, controls)?)
}

pub fn apply_runtime(hal: &Hal, controls: CpuControls) -> Result<()> {
    if controls.core_offset_mv.is_some() || controls.gt_offset_mv.is_some() {
        validate_voltage(controls.core_offset_mv, controls.gt_offset_mv)?;
    }
    let units = |v: Option<i32>| {
        v.map(|v| msr::offset_units(v as f64))
            .transpose()
            .map_err(|e| anyhow!(e))
    };
    runtime::apply(
        hal,
        runtime::Settings {
            pl1_w: controls.pl1_w,
            pl2_w: controls.pl2_w,
            core_offset_units: units(controls.core_offset_mv)?,
            gt_offset_units: units(controls.gt_offset_mv)?,
        },
    )
}

pub fn restore(hal: &Hal) -> Result<()> {
    let (patches, pl, voltages) = {
        let state = hal.recovery()?;
        (state.power.clone(), state.runtime_pl, state.voltage.clone())
    };
    let mut errors = Vec::new();
    if !patches.is_empty() {
        match restore_patches(hal, &patches) {
            Ok(_) => hal.recovery()?.power.clear(),
            Err(e) => errors.push(format!("保存值恢复：{e:#}")),
        }
    }
    if pl.is_some() || !voltages.is_empty() {
        match hal.open_msr() {
            Ok(io) => {
                if let Some(value) = pl {
                    match msr::restore_power(&io, value) {
                        Ok(()) => hal.recovery()?.runtime_pl = None,
                        Err(e) => errors.push(e),
                    }
                }
                for (plane, units) in voltages {
                    match runtime::voltage_if_changed(&io, plane, units) {
                        Ok(()) => {
                            hal.recovery()?.voltage.remove(&plane);
                        }
                        Err(e) => errors.push(e),
                    }
                }
            }
            Err(e) => errors.push(format!("运行值恢复：{e:#}")),
        }
    }
    if !errors.is_empty() {
        bail!("{}", errors.join("；"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_partial_invalid_and_inverted_power() {
        assert!(validate_pl(None, None).is_err());
        assert!(validate_pl(Some(f64::NAN), Some(80.0)).is_err());
        assert!(validate_pl(Some(90.0), Some(80.0)).is_err());
        assert!(validate_pl(Some(60.5), Some(80.0)).is_err());
        assert!(validate_pl(Some(60.0), Some(80.0)).is_ok());
    }
    #[test]
    fn patches_touch_only_the_requested_fields() {
        assert_eq!(
            power_patches(Some(65.0), None).unwrap(),
            vec![Patch::new(33, 65i32.to_le_bytes())]
        );
        assert!(voltage_patches(None, Some(1)).is_err());
    }
}
