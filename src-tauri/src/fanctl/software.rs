//! 软件自由曲线调度；所有入口在 HAL 控制锁内运行，写入不修改固件曲线表。
use crate::{
    fanctl::FanProfile,
    hal::{Hal, HardwareSnapshot, firmware},
};
use anyhow::{Context, Result, anyhow, bail, ensure};
use ccc_core::software_fan::{self, Controller, Curves};
use std::{collections::BTreeMap, time::Instant};

pub struct Session {
    controller: Controller,
    updated: Instant,
    gpu_required: bool,
}

fn temperature(sample: &HardwareSnapshot, gpu_required: bool) -> Result<f64> {
    let cpu = sample
        .fan
        .cpu
        .temp_c
        .filter(|v| v.is_finite() && (0.0..150.0).contains(v))
        .ok_or_else(|| anyhow!("CPU 温度不可用"))?;
    let gpu = sample.nvidia.as_ref().and_then(|g| g.temp_c);
    if gpu_required && gpu.is_none() {
        bail!("GPU 温度丢失");
    }
    if let Some(gpu) = gpu {
        if !gpu.is_finite() || !(0.0..150.0).contains(&gpu) {
            bail!("GPU 温度无效");
        }
    }
    let hottest = gpu.map_or(cpu, |gpu| cpu.max(gpu));
    if hottest >= 95.0 {
        bail!("温度达到 95°C 熔断阈值");
    }
    Ok(hottest)
}

impl Session {
    fn new(curves: Curves, sample: &HardwareSnapshot) -> Result<Self> {
        let gpu_required =
            curves.contains_key("gpu1") || sample.nvidia.as_ref().and_then(|g| g.temp_c).is_some();
        temperature(sample, gpu_required)?;
        let mut initial = BTreeMap::new();
        for (id, duty) in [
            ("cpu", sample.fan.cpu.duty_pct),
            ("gpu1", sample.fan.gpu1.duty_pct),
        ] {
            if let Some(duty) = duty {
                initial.insert(id.into(), duty);
            }
        }
        Ok(Self {
            controller: Controller::new(curves, &initial).map_err(|e| anyhow!(e))?,
            updated: Instant::now(),
            gpu_required,
        })
    }

    fn advance(
        &mut self,
        sample: &HardwareSnapshot,
        elapsed: f64,
        mut output: impl FnMut(&[u8; 256]) -> Result<()>,
    ) -> Result<()> {
        self.gpu_required |= sample.nvidia.as_ref().and_then(|g| g.temp_c).is_some();
        let hottest = temperature(sample, self.gpu_required)?;
        for packet in self
            .controller
            .step(hottest, elapsed)
            .map_err(|e| anyhow!(e))?
        {
            output(&packet)?;
        }
        Ok(())
    }
}

fn prepare(profile: &FanProfile, curves: &Curves) -> Result<()> {
    software_fan::validate(curves).map_err(|e| anyhow!(e))?;
    if !profile.software_supported {
        bail!(firmware::FAN_CONTROL_UNVERIFIED);
    }
    if curves
        .keys()
        .any(|id| !["cpu", "gpu1"].contains(&id.as_str()))
    {
        bail!("已核对的本机仅开放 CPU 与 GPU 1 风扇");
    }
    Ok(())
}

pub fn validate(hal: &Hal, curves: &Curves) -> Result<()> {
    super::dust::ensure_software_allowed(hal)?;
    validate_configuration(hal, curves)
}
pub(crate) fn validate_configuration(hal: &Hal, curves: &Curves) -> Result<()> {
    ensure!(
        hal.software_fan_supported(),
        firmware::FAN_CONTROL_UNVERIFIED
    );
    prepare(&super::read_profile(hal)?, curves)?;
    Session::new(curves.clone(), &hal.snapshot()).map(|_| ())
}
pub(crate) fn validate_points(hal: &Hal, curves: &Curves) -> Result<()> {
    ensure!(
        hal.software_fan_supported(),
        firmware::FAN_CONTROL_UNVERIFIED
    );
    prepare(&super::read_profile(hal)?, curves).map(|_| ())
}

pub(crate) fn active(hal: &Hal) -> Result<bool> {
    Ok(
        hal.fan_recovery_pending()
            || has_progress(hal)?
            || hal.software_fan_release_unverified()?,
    )
}

fn has_progress(hal: &Hal) -> Result<bool> {
    Ok(hal
        .software_progress
        .lock()
        .map_err(|_| anyhow!("软件曲线进度异常"))?
        .is_some())
}

pub fn read(hal: &Hal) -> Result<Option<Curves>> {
    if !has_progress(hal)? {
        return Ok(None);
    }
    Ok(hal
        .software_fan
        .lock()
        .map_err(|_| anyhow!("软件曲线状态异常"))?
        .as_ref()
        .map(|s| s.controller.curves.clone()))
}

pub(crate) fn read_for_handoff(hal: &Hal) -> Result<Option<Curves>> {
    let curves = read(hal)?;
    ensure!(
        !hal.fan_recovery_pending() && (curves.is_some() || !active(hal)?),
        "尚未确认风扇已退出软件调速，请等待恢复"
    );
    Ok(curves)
}

#[cfg(test)]
pub(crate) fn inject_test_session(hal: &Hal, curves: Curves) -> Result<()> {
    let session = Session::new(curves, &hal.snapshot())?;
    *hal.software_fan.lock().unwrap() = Some(session);
    *hal.software_progress.lock().unwrap() = Some(Instant::now());
    Ok(())
}

fn stop_progress(hal: &Hal) -> Result<bool> {
    Ok(hal
        .software_progress
        .lock()
        .map_err(|_| anyhow!("软件曲线进度异常"))?
        .take()
        .is_some())
}

fn discard_session(hal: &Hal) -> Result<bool> {
    Ok(hal
        .software_fan
        .lock()
        .map_err(|_| anyhow!("软件曲线状态异常"))?
        .take()
        .is_some())
}

pub fn discard(hal: &Hal) -> Result<bool> {
    let running = stop_progress(hal)?;
    Ok(discard_session(hal)? || running)
}

fn release(hal: &Hal) -> Result<()> {
    // Keep the progress deadline while the foreground writer waits for release readback.
    if !discard_session(hal)? && !hal.software_fan_release_unverified()? {
        return Ok(());
    }
    let result = hal.with_dchu(|d| {
        ensure!(hal.reviewed_dsdt, firmware::FAN_CONTROL_UNVERIFIED);
        d.set_u32(0x69, firmware::DIRECT_FAN_MASK)?;
        firmware::verify_direct_release(d, hal.direct_fan_reader())
    });
    if let Err(error) = result {
        hal.latch_software_fan_fault();
        if hal.software_fan_supported() {
            hal.request_fan_recovery()?;
        } else {
            stop_progress(hal)?;
        }
        return Err(error).context("尚未确认风扇已退出软件调速");
    }
    stop_progress(hal)?;
    ensure!(!hal.fan_recovery_pending(), "独立风扇监护正在恢复");
    Ok(())
}

pub fn stop(hal: &Hal) -> Result<()> {
    release(hal)?;
    hal.record_software_fan_stopped()
}

pub(crate) fn stop_safely(hal: &Hal) -> Result<()> {
    hal.pulse_fan_guard()?;
    release(hal)?;
    hal.disarm_fan_guard()?;
    hal.record_software_fan_stopped()
        .context("风扇已退出软件调速，未能保存恢复记录")
}

fn recover(hal: &Hal, error: anyhow::Error) -> anyhow::Error {
    let result = (|| {
        discard_session(hal)?;
        hal.latch_software_fan_fault();
        ensure!(!hal.fan_recovery_pending(), "独立风扇监护正在恢复");
        hal.with_dchu(|d| crate::watchdog::restore_auto(d, hal.direct_fan_reader()))?;
        stop_progress(hal)?;
        hal.disarm_fan_guard()
    })();
    match result {
        Ok(()) => match hal.record_software_fan_stopped() {
            Ok(()) => anyhow!("{error:#}；已核实风扇恢复自动控制"),
            Err(record) => anyhow!("{error:#}；已核实风扇恢复自动控制，恢复记录未提交：{record:#}"),
        },
        Err(restore) => {
            if active(hal).unwrap_or(true) {
                if let Err(retry) = hal.request_fan_recovery() {
                    log::error!("[watchdog] 无法安排自动恢复：{retry:#}");
                }
            }
            anyhow!("{error:#}；自动恢复失败：{restore:#}")
        }
    }
}

fn write_step(hal: &Hal, session: &mut Session, elapsed: f64) -> Result<()> {
    let reader = hal
        .direct_fan_reader()
        .ok_or_else(|| anyhow!(firmware::FAN_CONTROL_UNVERIFIED))?;
    super::dust::ensure_software_allowed(hal)?;
    crate::powerctl::mode::boost::ensure_fan_allowed(
        hal,
        crate::powerctl::mode::boost::FanTarget::Software,
    )?;
    let snapshot = hal.snapshot();
    session.advance(&snapshot, elapsed, |packet| {
        hal.pulse_fan_guard()?;
        hal.with_dchu(|d| {
            ensure!(has_progress(hal)?, "软件控制已被监护停止");
            d.set_package(4, packet)?;
            let channel = usize::from(packet[1] - 1);
            ensure!(
                reader(d)?[channel] == firmware::DirectControl::Manual(packet[2]),
                "风扇通道 {} 接管或目标占空比回读不一致",
                packet[1]
            );
            Ok(())
        })
    })?;
    let mut progress = hal
        .software_progress
        .lock()
        .map_err(|_| anyhow!("软件曲线进度异常"))?;
    if progress.is_none() {
        bail!("软件控制已被监护停止");
    }
    let now = Instant::now();
    session.updated = now;
    *progress = Some(now);
    Ok(())
}

pub fn apply(hal: &Hal, curves: &Curves) -> Result<()> {
    apply_inner(hal, curves, false)
}

pub(crate) fn apply_manual(hal: &Hal, curves: &Curves) -> Result<()> {
    apply_inner(hal, curves, true)
}

fn apply_inner(hal: &Hal, curves: &Curves, manual: bool) -> Result<()> {
    hal.ensure_software_fan_start_allowed(manual)?;
    super::dust::ensure_software_allowed(hal)?;
    ensure!(
        hal.software_fan_supported(),
        firmware::FAN_CONTROL_UNVERIFIED
    );
    crate::powerctl::mode::boost::ensure_fan_allowed(
        hal,
        crate::powerctl::mode::boost::FanTarget::Software,
    )?;
    let current = super::read_profile(hal)?;
    prepare(&current, curves)?;
    let mut session = Session::new(curves.clone(), &hal.snapshot())?;
    let before = super::checkpoint(hal, false)?;
    super::checkpoint::remember_baseline(hal, &before)?;
    hal.pulse_fan_guard()?;
    stop(hal)?;
    hal.with_dchu(|d| firmware::verify_direct_release(d, hal.direct_fan_reader()))?;
    hal.arm_fan_guard()?;
    if let Err(error) = hal.record_software_fan_started(manual) {
        return match hal.disarm_fan_guard() {
            Ok(()) => Err(error),
            Err(disarm) => Err(anyhow!("{error:#}；解除监护失败：{disarm:#}")),
        };
    }
    let result = (|| {
        *hal.software_progress
            .lock()
            .map_err(|_| anyhow!("软件曲线进度异常"))? = Some(Instant::now());
        write_step(hal, &mut session, 0.5)?;
        *hal.software_fan
            .lock()
            .map_err(|_| anyhow!("软件曲线状态异常"))? = Some(session);
        Ok(())
    })();
    result.map_err(|error| recover(hal, error))?;
    hal.set_safety_status("软件曲线 · 监护中".into())
}

pub fn tick(hal: &Hal) -> Result<()> {
    let _write = hal.begin_write()?;
    let mut slot = hal
        .software_fan
        .lock()
        .map_err(|_| anyhow!("软件曲线状态异常"))?;
    let Some(session) = slot.as_mut() else {
        return Ok(());
    };
    let result = write_step(hal, session, session.updated.elapsed().as_secs_f64());
    drop(slot);
    if let Err(error) = result {
        let error = recover(hal, error);
        hal.set_safety_status(format!("软件曲线已停止：{error:#}"))?;
        return Err(error);
    }
    Ok(())
}

#[cfg(test)]
#[path = "software/tests.rs"]
mod tests;
