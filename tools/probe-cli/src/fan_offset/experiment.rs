use super::{FanOffsetArgs, Phase, mode_name, stopped};
use crate::dchu::Dchu;
use anyhow::{Context, Result, bail, ensure};
use ccc_core::{
    fan::{decode_oem_fan_mode, oem_duty_byte},
    fan_sensors::{FanReadings, fan_channels},
};
use std::{
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Copy)]
pub(super) struct Baseline {
    pub mode: u8,
    pub offset: u8,
    pub channels: u8,
}

pub(super) trait FanIo {
    fn read_package(&self, command: i32) -> Result<[u8; 256]>;
    fn dust_running(&self) -> Result<Option<bool>>;
    fn read_offset(&self) -> Result<u8>;
    fn set_mode(&self, mode: u8) -> Result<()>;
    fn set_offset(&self, raw: u8) -> Result<()>;
}

impl FanIo for Dchu {
    fn read_package(&self, command: i32) -> Result<[u8; 256]> {
        self.read_package(command)
    }

    fn dust_running(&self) -> Result<Option<bool>> {
        self.dust_running()
    }

    fn read_offset(&self) -> Result<u8> {
        self.fan_offset_raw()
    }

    fn set_mode(&self, mode: u8) -> Result<()> {
        self.set_fan_mode(mode)
    }

    fn set_offset(&self, raw: u8) -> Result<()> {
        self.set_fan_offset(raw)
    }
}

pub(super) trait ActiveLoad {
    fn refresh(&self, sampled_at: Instant) -> Result<()>;
}

#[derive(Clone, Copy)]
pub(super) struct Sample {
    pub cpu_duty: f64,
    pub cpu_rpm: f64,
    pub cpu_temp: f64,
    pub gpu_duty: f64,
    pub gpu_rpm: f64,
    pub gpu_temp: f64,
}

pub(super) struct Row {
    pub at: u64,
    pub phase: &'static str,
    pub sampled: bool,
    pub live: u8,
    pub sample: Sample,
}

struct Observation {
    mode_bits: u8,
    sample: Sample,
    taken_at: Instant,
}

fn decode_sample(packet: &[u8; 256], channels: u8) -> Result<Sample> {
    const THERMAL_ABORT_C: f64 = 95.0;
    let readings = FanReadings::decode(packet, channels).context("风扇传感数据无效")?;
    let temperature = |label: &str, value: Option<f64>| -> Result<f64> {
        let value = value.with_context(|| format!("{label} 温度失效"))?;
        ensure!(
            value < THERMAL_ABORT_C,
            "{label} 温度 {value:.0}°C 已达 {THERMAL_ABORT_C:.0}°C 中止阈值"
        );
        Ok(value)
    };
    for (mask, name, reading) in [(4, "GPU 2", readings.gpu2), (8, "系统风扇", readings.sys)] {
        if channels & mask != 0 {
            temperature(name, reading.temp_c)?;
        }
    }
    Ok(Sample {
        cpu_temp: temperature("CPU", readings.cpu.temp_c)?,
        gpu_temp: temperature("GPU 1", readings.gpu1.temp_c)?,
        cpu_duty: readings.cpu.duty_pct.context("CPU 占空比缺失")?,
        cpu_rpm: readings.cpu.rpm.context("CPU 转速缺失")?,
        gpu_duty: readings.gpu1.duty_pct.context("GPU 1 占空比缺失")?,
        gpu_rpm: readings.gpu1.rpm.context("GPU 1 转速缺失")?,
    })
}

fn observe(io: &impl FanIo, expected_mode: u8, channels: u8) -> Result<Observation> {
    let info = io.read_package(13).context("风扇档位和通道读取失败")?;
    ensure!(
        fan_channels(&info) == Some(channels),
        "风扇通道已变化或不可读"
    );
    let bits = info[14];
    let mode = decode_oem_fan_mode(bits).map_err(anyhow::Error::msg)?;
    ensure!(
        mode == expected_mode,
        "即时档位 {bits:#04x} 与预期的「{}」不符",
        mode_name(expected_mode)
    );
    let taken_at = Instant::now();
    let packet = io.read_package(12).context("风扇传感读取失败")?;
    let sample = decode_sample(&packet, channels)?;
    ensure!(
        io.dust_running().context("除尘状态读取失败")? != Some(true),
        "固件正在除尘"
    );
    Ok(Observation {
        mode_bits: bits,
        taken_at,
        sample,
    })
}

pub(super) fn run<L: ActiveLoad>(
    io: &impl FanIo,
    before: Baseline,
    plan: &[Phase; 6],
    args: &FanOffsetArgs,
    mut record: impl FnMut(&Row) -> Result<()>,
    start_load: impl FnOnce(Instant) -> Result<L>,
) -> Result<Option<Vec<Row>>> {
    let mut wrote = false;
    let outcome = run_phases(io, before, plan, args, &mut record, start_load, &mut wrote);
    let recovery = if wrote { restore(io, before) } else { Ok(()) };
    match (outcome, recovery) {
        (outcome, Ok(())) => outcome,
        (Ok(_), Err(error)) => Err(error),
        (Err(error), Err(recovery)) => Err(error.context(format!("恢复失败：{recovery:#}"))),
    }
}

fn run_phases<L: ActiveLoad>(
    io: &impl FanIo,
    before: Baseline,
    plan: &[Phase; 6],
    args: &FanOffsetArgs,
    record: &mut impl FnMut(&Row) -> Result<()>,
    start_load: impl FnOnce(Instant) -> Result<L>,
    wrote: &mut bool,
) -> Result<Option<Vec<Row>>> {
    if stopped() {
        return Ok(None);
    }
    let initial = observe(io, before.mode, before.channels).context("启动前复核失败")?;
    ensure!(
        io.read_offset()? == before.offset,
        "启动前发现即时偏移已变化"
    );
    let load = start_load(initial.taken_at)?;
    let mut rows = Vec::new();
    let mut mode = before.mode;
    for phase in plan {
        if stopped() {
            return Ok(None);
        }
        let fresh = observe(io, mode, before.channels)
            .with_context(|| format!("阶段 {} 启动前复核失败", phase.name))?;
        load.refresh(fresh.taken_at)?;
        if stopped() {
            return Ok(None);
        }
        *wrote = true;
        if let Some(next) = phase.mode {
            io.set_mode(next)?;
            mode = next;
        }
        if stopped() {
            return Ok(None);
        }
        load.refresh(fresh.taken_at)?;
        io.set_offset(oem_duty_byte(f64::from(phase.offset)))?;
        println!("阶段 {}@{}%…", phase.name, phase.offset);
        let started = Instant::now();
        let mut samples = 0;
        while !stopped() && started.elapsed().as_secs() < args.settle + args.window {
            let tick = Instant::now();
            let reading = observe(io, mode, before.channels)
                .with_context(|| format!("阶段 {} 取样中止", phase.name))?;
            load.refresh(reading.taken_at)?;
            let at = tick.duration_since(started).as_secs();
            let row = Row {
                at,
                phase: phase.name,
                sampled: at >= args.settle,
                live: reading.mode_bits,
                sample: reading.sample,
            };
            record(&row)?;
            samples += usize::from(row.sampled);
            rows.push(row);
            if let Some(rest) = Duration::from_secs(1).checked_sub(tick.elapsed()) {
                thread::sleep(rest);
            }
        }
        println!("…取样 {samples} 条");
    }
    Ok((!stopped()).then_some(rows))
}

fn restore(io: &impl FanIo, before: Baseline) -> Result<()> {
    let offset = io.set_offset(before.offset);
    let mode = io.set_mode(before.mode);
    let actual_offset = io.read_offset();
    let actual_mode = io
        .read_package(13)
        .and_then(|info| decode_oem_fan_mode(info[14]).map_err(anyhow::Error::msg));
    if offset.is_ok()
        && mode.is_ok()
        && actual_offset
            .as_ref()
            .is_ok_and(|&raw| raw == before.offset)
        && actual_mode.as_ref().is_ok_and(|&mode| mode == before.mode)
    {
        println!(
            "已读回原档位「{}」和 EC 偏移 {:#04x}。",
            mode_name(before.mode),
            before.offset
        );
        Ok(())
    } else {
        bail!(
            "恢复未确认。原档位「{}」、EC 偏移 {:#04x}。偏移发送 {offset:?}，档位发送 {mode:?}，偏移回读 {actual_offset:?}，档位回读 {actual_mode:?}",
            mode_name(before.mode),
            before.offset,
        );
    }
}

#[cfg(test)]
mod tests;
