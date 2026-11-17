//! 即时 CPU 目标、预检快照及原始编码恢复；与 OEM 保存字段分开。
use crate::hal::{Hal, msr::MsrIo};
use anyhow::{Result, anyhow, bail};
use ccc_core::msr::{self, MsrAccess};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct Settings {
    pub pl1_w: Option<f64>,
    pub pl2_w: Option<f64>,
    pub core_offset_units: Option<i16>,
    pub gt_offset_units: Option<i16>,
}

pub fn validate(target: &Settings) -> Result<()> {
    if [target.pl1_w, target.pl2_w]
        .into_iter()
        .flatten()
        .any(|v| !v.is_finite() || !(super::PL_MIN_W..=super::PL_MAX_W).contains(&v))
    {
        bail!("即时功耗范围 35–157 W");
    }
    super::validate_pl_order(target.pl1_w, target.pl2_w)?;
    let min = msr::offset_units(super::UV_MIN_MV as f64).map_err(|e| anyhow!(e))?;
    let max = msr::offset_units(super::UV_MAX_MV as f64).map_err(|e| anyhow!(e))?;
    if target
        .core_offset_units
        .is_some_and(|v| !(min..=max).contains(&v))
        || target
            .gt_offset_units
            .is_some_and(|v| !(min..=0).contains(&v))
    {
        bail!("即时电压编码超出核心或核显电压偏移的应用范围");
    }
    if *target == Settings::default() {
        bail!("即时 CPU 配置为空");
    }
    Ok(())
}

#[derive(Clone, Debug)]
struct Snapshot {
    pl: Option<u64>,
    voltage: BTreeMap<u64, i16>,
}

fn check_power(io: &impl MsrAccess, target: &Settings) -> Result<(u64, bool)> {
    let before = io.read(msr::MSR_PKG_POWER_LIMIT).map_err(|e| anyhow!(e))?;
    let unit = msr::power_unit(io.read(msr::MSR_RAPL_POWER_UNIT).map_err(|e| anyhow!(e))?);
    let (a, b) = msr::decode_power_limits(before, unit);
    super::validate_pl_order(target.pl1_w.or(Some(a)), target.pl2_w.or(Some(b)))?;
    let encoded = msr::encode_power_limit(before, target.pl1_w, target.pl2_w, unit)
        .map_err(|e| anyhow!(e))?;
    let mask = msr::power_field_mask(target.pl1_w.is_some(), target.pl2_w.is_some());
    let changed = before & mask != encoded & mask;
    if changed && before & (1 << 63) != 0 {
        bail!("MSR 0x610 bit63 已锁定，未执行目标写入");
    }
    Ok((before, changed))
}

fn capture(io: &impl MsrAccess, target: &Settings) -> Result<Snapshot> {
    validate(target)?;
    let pl = if target.pl1_w.is_some() || target.pl2_w.is_some() {
        Some(check_power(io, target)?.0)
    } else {
        None
    };
    let mut voltage = BTreeMap::new();
    for (plane, value) in [(0, target.core_offset_units), (1, target.gt_offset_units)] {
        if value.is_some() {
            voltage.insert(
                plane,
                msr::read_voltage_units(io, plane).map_err(|e| anyhow!(e))?,
            );
        }
    }
    Ok(Snapshot { pl, voltage })
}

fn commit(io: &impl MsrAccess, target: &Settings) -> Result<()> {
    if target.pl1_w.is_some() || target.pl2_w.is_some() {
        if check_power(io, target)?.1 {
            msr::apply_power(io, target.pl1_w, target.pl2_w).map_err(|e| anyhow!(e))?;
        }
    }
    for (plane, value) in [(0, target.core_offset_units), (1, target.gt_offset_units)] {
        if let Some(value) = value {
            voltage_if_changed(io, plane, value).map_err(|e| anyhow!(e))?;
        }
    }
    Ok(())
}

fn restore(io: &impl MsrAccess, snapshot: &Snapshot) -> Result<()> {
    let mut errors = Vec::new();
    if let Some(pl) = snapshot.pl {
        if let Err(error) = msr::restore_power(io, pl) {
            errors.push(format!("功耗上限恢复：{error}"));
        }
    }
    for (&plane, &units) in &snapshot.voltage {
        if let Err(error) = voltage_if_changed(io, plane, units) {
            errors.push(format!("电压域 {plane} 恢复：{error}"));
        }
    }
    if !errors.is_empty() {
        bail!(errors.join("；"));
    }
    Ok(())
}

pub(super) fn voltage_if_changed(
    io: &impl MsrAccess,
    plane: u64,
    units: i16,
) -> std::result::Result<(), String> {
    if msr::read_voltage_units(io, plane)? == units {
        return Ok(());
    }
    msr::write_voltage_units(io, plane, units)
}

pub struct Plan {
    io: MsrIo,
    before: Snapshot,
    target: Settings,
}

impl Plan {
    pub fn prepare(hal: &Hal, target: Settings) -> Result<Self> {
        validate(&target)?;
        let io = hal.open_msr()?;
        let before = capture(&io, &target)?;
        Ok(Self { io, before, target })
    }
    pub fn remember_baseline(&self, hal: &Hal) -> Result<()> {
        let mut recovery = hal.recovery()?;
        if let Some(pl) = self.before.pl {
            recovery.runtime_pl.get_or_insert(pl);
        }
        for (&plane, &units) in &self.before.voltage {
            recovery.voltage.entry(plane).or_insert(units);
        }
        Ok(())
    }
    /// 调用方持有控制锁，并负责在跨域事务失败时调用 restore。
    pub fn execute(&self, hal: &Hal) -> Result<()> {
        self.remember_baseline(hal)?;
        commit(&self.io, &self.target)
    }
    pub fn restore(&self) -> Result<()> {
        restore(&self.io, &self.before)
    }
}

pub fn apply(hal: &Hal, target: Settings) -> Result<()> {
    let plan = Plan::prepare(hal, target)?;
    if let Err(error) = plan.execute(hal) {
        bail!(msr::with_recovery(
            format!("{error:#}"),
            plan.restore().map_err(|e| format!("{e:#}"))
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "runtime/tests.rs"]
mod tests;
