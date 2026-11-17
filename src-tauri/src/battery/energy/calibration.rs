//! 校准临时接管共享字段；保存计划仍由energy-policy.json唯一拥有。
use super::*;
use crate::battery::{Io as BatteryIo, calibration::Baseline, native::Native as Battery};

pub(in crate::battery) fn capture(hal: &Hal, path: &std::path::Path) -> Result<Baseline> {
    let power_scheme = crate::battery::calibration::power_scheme(hal.system_power.active()?);
    let stop_threshold = Battery { hal }
        .diagnostics()?
        .context("未提供原厂校准诊断")?
        .stop_charging_threshold;
    let observed = observe(&native::Native { hal }, &FileStore(path.into()))?;
    let baseline = if let Some((snapshot, record)) = observed {
        if record.is_some_and(|r| r.pending.is_some()) {
            bail!("节能设置尚需恢复，未开始校准");
        }
        Baseline {
            stop_threshold,
            power_scheme,
            charge: snapshot.charge.clone(),
            energy: Some(snapshot),
        }
    } else {
        Baseline {
            stop_threshold,
            power_scheme,
            charge: Battery { hal }
                .charge()?
                .context("没有可捕获的充电恢复策略")?,
            energy: None,
        }
    };
    baseline.validate()?;
    Ok(baseline)
}
pub(in crate::battery) fn restore(hal: &Hal, baseline: &Baseline) -> Result<()> {
    baseline.validate()?;
    let io = native::Native { hal };
    if let Some(snapshot) = &baseline.energy {
        if io.capability()?.as_ref() != Some(&snapshot.capability) {
            bail!("原厂节能能力已变化，保留校准恢复记录");
        }
        recover(&io, snapshot)
    } else {
        io.restore_charge(&baseline.charge)
    }
}
