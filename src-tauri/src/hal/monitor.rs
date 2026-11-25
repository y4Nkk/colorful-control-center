use super::{Hal, HardwareSnapshot, memory, nvidia::Nvidia, oem_dchu::Dchu};
use anyhow::{Context, Result};
use ccc_core::fan_sensors::{FanReadings, fan_channels};

impl Hal {
    pub(crate) fn tray_temperatures(&self) -> (Option<f64>, Option<f64>) {
        let cpu = self
            .with_dchu(|io| io.read_package(12))
            .ok()
            .and_then(|page| FanReadings::decode(&page, 0))
            .and_then(|readings| readings.cpu.temp_c);
        let gpu = self
            .sample_nvidia(Nvidia::sample_temperature)
            .ok()
            .flatten()
            .and_then(|reading| (!self.gpu_suspended()).then_some(reading.temp_c).flatten());
        (cpu, gpu)
    }

    pub fn display_snapshot(&self) -> HardwareSnapshot {
        let mut snapshot = self.sample_hardware(true);
        let reading = self
            .cpu_utility
            .lock()
            .map_err(|_| anyhow::anyhow!("CPU效用采样状态异常"))
            .and_then(|mut sampler| {
                sampler.sample(std::time::Instant::now(), snapshot.sampled_at_ms)
            });
        match reading {
            Ok(value) => snapshot.cpu_utility = value,
            Err(error) => snapshot
                .warnings
                .push(format!("CPU效用读取失败：{error:#}")),
        }
        snapshot
    }
    pub fn snapshot(&self) -> HardwareSnapshot {
        self.sample_hardware(false)
    }
    fn sample_hardware(&self, display: bool) -> HardwareSnapshot {
        let mut snapshot = HardwareSnapshot {
            sampled_at_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0),
            ..Default::default()
        };
        if let Err(error) = self.with_dchu(|io| {
            if display {
                match crate::powerctl::mode::current_mode(io) {
                    Ok(mode) => snapshot.oem_power_mode = Some(mode),
                    Err(error) => snapshot
                        .warnings
                        .push(format!("当前模式读取失败：{error:#}")),
                }
            }
            sample_fans(io, &mut snapshot)?;
            snapshot.oem_memory = memory::read(io, &mut snapshot.warnings);
            Ok(())
        }) {
            snapshot.warnings.push(format!("传感读取失败：{error:#}"));
        }
        match self.sample_nvidia(Nvidia::sample) {
            Ok(value) => snapshot.nvidia = value,
            Err(error) => snapshot
                .warnings
                .push(format!("GPU 传感读取失败：{error:#}")),
        }
        snapshot.nvidia_suspended = self.gpu_suspended();
        if snapshot.nvidia_suspended {
            snapshot.nvidia = None;
        }
        snapshot
    }
}

fn sample_fans(io: &Dchu, snapshot: &mut HardwareSnapshot) -> Result<()> {
    let package = io.read_package(12)?;
    snapshot.dchu_available = true;
    let channels = match io
        .read_package(13)
        .and_then(|info| fan_channels(&info).context("风扇数量或存在位无效"))
    {
        Ok(channels) => channels,
        Err(error) => {
            snapshot
                .warnings
                .push(format!("风扇通道读取失败：{error:#}"));
            0
        }
    };
    if let Some(readings) = FanReadings::decode(&package, channels) {
        snapshot.fan = readings;
        if snapshot.fan.cpu.temp_c.is_none() {
            snapshot.warnings.push("CPU 温度不可用".into());
        }
    } else {
        snapshot.warnings.push("风扇传感数据无效".into());
    }
    Ok(())
}
