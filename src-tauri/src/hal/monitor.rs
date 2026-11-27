use super::{Hal, HardwareSnapshot, memory, nvidia::Nvidia, oem_dchu::Dchu};
use anyhow::{Context, Result};
use ccc_core::fan_sensors::{FanReadings, fan_channels};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const DCHU_RETRY: Duration = Duration::from_secs(5);
const FAN_CHANNEL_TTL: Duration = Duration::from_secs(30);

#[derive(Default)]
pub(crate) struct DisplaySampling {
    sensors: DisplaySensors,
    retry_after: Option<Instant>,
}

#[derive(Default)]
struct DisplaySensors {
    channels: TimedReading<u8>,
    memory: memory::DisplayCache,
}

#[derive(Default)]
pub(super) struct TimedReading<T>(Option<(Instant, T)>);

impl<T: Copy> TimedReading<T> {
    pub(super) fn read(
        &mut self,
        now: Instant,
        lifetime: Duration,
        read: impl FnOnce() -> Result<T>,
    ) -> Result<T> {
        if let Some((at, value)) = self.0
            && now
                .checked_duration_since(at)
                .is_some_and(|age| age < lifetime)
        {
            return Ok(value);
        }
        self.0 = None;
        let value = read()?;
        self.0 = Some((now, value));
        Ok(value)
    }
}

impl DisplaySampling {
    fn sample_dchu(
        &mut self,
        hal: &Hal,
        now: Instant,
        snapshot: &mut HardwareSnapshot,
    ) -> Result<()> {
        let may_load = self.retry_after.is_none_or(|at| now >= at);
        let mut connected = false;
        let result = hal.with_dchu_connection(may_load, |io| {
            connected = true;
            if self.retry_after.take().is_some() {
                self.sensors = DisplaySensors::default();
            }
            sample_mode(io, snapshot);
            let package = io.read_package(12)?;
            let channels = self
                .sensors
                .channels
                .read(now, FAN_CHANNEL_TTL, || read_channels(io));
            record_fans(snapshot, &package, channels);
            snapshot.oem_memory = self.sensors.memory.read(io, now, &mut snapshot.warnings);
            Ok(())
        });
        if result.is_err() {
            self.sensors = DisplaySensors::default();
            if !connected && may_load {
                self.retry_after = Some(now + DCHU_RETRY);
            }
        }
        result
    }
}

impl Hal {
    pub(crate) fn tray_temperatures(&self) -> (Option<f64>, Option<f64>) {
        let cpu = self
            .with_dchu(|io| io.read_package(12))
            .ok()
            .and_then(|page| FanReadings::decode(&page, 0))
            .and_then(|readings| readings.cpu.temp_c);
        let gpu = self.read_gpu_temperature().ok().flatten();
        (cpu, gpu)
    }

    fn read_gpu_temperature(&self) -> Result<Option<f64>> {
        #[cfg(test)]
        if !self.gpu_suspended()
            && let Some(source) = &self.test_nvidia
        {
            return source().map(|value| value.and_then(|reading| reading.temp_c));
        }
        let temperature = self
            .sample_nvidia(true, Nvidia::sample_temperature)?
            .flatten();
        Ok((!self.gpu_suspended()).then_some(temperature).flatten())
    }

    pub(crate) fn read_gpu_details(
        &self,
        connection_id: u32,
    ) -> Result<Option<super::nvidia::GpuDetails>> {
        let details = self.sample_nvidia(false, |device| device.sample_details(connection_id))?;
        Ok((!self.gpu_suspended())
            .then_some(details)
            .flatten()
            .flatten())
    }

    pub(crate) fn display_snapshot(
        &self,
        sampling: &mut DisplaySampling,
        now: Instant,
    ) -> HardwareSnapshot {
        let mut snapshot = new_snapshot();
        let dchu = sampling.sample_dchu(self, now, &mut snapshot);
        record_transport(&mut snapshot, dchu);
        self.sample_snapshot_gpu(&mut snapshot);
        let reading = self
            .cpu_utility
            .lock()
            .map_err(|_| anyhow::anyhow!("CPU效用采样状态异常"))
            .and_then(|mut sampler| sampler.sample(Instant::now(), snapshot.sampled_at_ms));
        match reading {
            Ok(value) => snapshot.cpu_utility = value,
            Err(error) => snapshot
                .warnings
                .push(format!("CPU效用读取失败：{error:#}")),
        }
        snapshot
    }

    pub fn snapshot(&self) -> HardwareSnapshot {
        let mut snapshot = new_snapshot();
        let dchu = self.with_dchu(|io| {
            let package = io.read_package(12)?;
            record_fans(&mut snapshot, &package, read_channels(io));
            snapshot.oem_memory = memory::read(io, &mut snapshot.warnings);
            Ok(())
        });
        record_transport(&mut snapshot, dchu);
        self.sample_snapshot_gpu(&mut snapshot);
        snapshot
    }

    pub(crate) fn reset_monitoring_after_resume(&self) {
        if let Ok(mut sampler) = self.cpu_utility.lock() {
            sampler.reset();
        }
        if let Ok(mut nvidia) = self.nvidia.lock() {
            *nvidia = super::nvidia::Provider::default();
        }
    }

    fn sample_snapshot_gpu(&self, snapshot: &mut HardwareSnapshot) {
        match self.read_gpu_snapshot() {
            Ok(value) => snapshot.nvidia = value,
            Err(error) => snapshot
                .warnings
                .push(format!("GPU 传感读取失败：{error:#}")),
        }
        snapshot.nvidia_suspended = self.gpu_suspended();
        if snapshot.nvidia_suspended {
            snapshot.nvidia = None;
        }
    }

    fn read_gpu_snapshot(&self) -> Result<Option<super::NvidiaSnapshot>> {
        #[cfg(test)]
        if !self.gpu_suspended()
            && let Some(source) = &self.test_nvidia
        {
            return source();
        }
        self.sample_nvidia(true, Nvidia::sample)
    }
}

fn new_snapshot() -> HardwareSnapshot {
    HardwareSnapshot {
        sampled_at_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis() as u64)
            .unwrap_or(0),
        ..Default::default()
    }
}

fn sample_mode(io: &Dchu, snapshot: &mut HardwareSnapshot) {
    match crate::powerctl::mode::current_mode(io) {
        Ok(mode) if ccc_core::oem_events::is_power_mode(mode) => {
            snapshot.oem_power_mode = Some(mode)
        }
        Ok(mode) => snapshot
            .warnings
            .push(format!("当前模式编号 {mode} 未识别")),
        Err(error) => snapshot
            .warnings
            .push(format!("当前模式读取失败：{error:#}")),
    }
}

fn read_channels(io: &Dchu) -> Result<u8> {
    fan_channels(&io.read_package(13)?).context("风扇数量或存在位无效")
}

fn record_fans(snapshot: &mut HardwareSnapshot, package: &[u8; 256], channels: Result<u8>) {
    snapshot.dchu_available = true;
    let channels = channels.unwrap_or_else(|error| {
        snapshot
            .warnings
            .push(format!("风扇通道读取失败：{error:#}"));
        0
    });
    if let Some(readings) = FanReadings::decode(package, channels) {
        snapshot.fan = readings;
        if snapshot.fan.cpu.temp_c.is_none() {
            snapshot.warnings.push("CPU 温度不可用".into());
        }
    } else {
        snapshot.warnings.push("风扇传感数据无效".into());
    }
}

fn record_transport(snapshot: &mut HardwareSnapshot, result: Result<()>) {
    if let Err(error) = result {
        snapshot.warnings.push(format!("传感读取失败：{error:#}"));
    }
}
