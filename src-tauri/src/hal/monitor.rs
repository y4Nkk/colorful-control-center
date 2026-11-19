use super::{FanSnapshot, Hal, HardwareSnapshot, memory};

impl Hal {
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
            let package = io.read_package(12)?;
            decode_fans(&mut snapshot, &package);
            snapshot.dchu_available = true;
            snapshot.oem_memory = memory::read(io, &mut snapshot.warnings);
            Ok(())
        }) {
            snapshot.warnings.push(format!("传感读取失败：{error:#}"));
        }
        match self.sample_nvidia() {
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

fn decode_fans(snapshot: &mut HardwareSnapshot, package: &[u8; 256]) {
    if package.iter().all(|b| *b == 0) || package.iter().all(|b| *b == 255) {
        snapshot.warnings.push("风扇传感数据无效".into());
        return;
    }
    let rpm = |at: usize| {
        ccc_core::fan::oem_tachometer_rpm(u16::from_be_bytes([package[at], package[at + 1]]))
    };
    let duty = |at: usize| package[at] as f64 * 100.0 / 255.0;
    snapshot.fan = FanSnapshot {
        cpu_rpm: Some(rpm(2)),
        cpu_duty_pct: Some(duty(16)),
        gpu_rpm: Some(rpm(4)),
        gpu_duty_pct: Some(duty(19)),
    };
    let raw = package[18] as f64;
    if raw > 0.0 && raw < 150.0 {
        snapshot.cpu_remote_raw = Some(raw);
        // OEM FAN.GetTDP() 固定返回 "0"，CalCPUTemp 的此分支直接返回 raw。
        snapshot.cpu_temp_c = Some(raw);
    } else {
        snapshot.warnings.push("CPU 温度不可用".into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_packet_never_fabricates_temperature() {
        let mut snapshot = HardwareSnapshot::default();
        decode_fans(&mut snapshot, &[0; 256]);
        assert!(snapshot.cpu_temp_c.is_none());
        assert!(snapshot.fan.cpu_rpm.is_none());
        assert!(!snapshot.warnings.is_empty());
    }
    #[test]
    fn stopped_fan_is_zero_not_missing() {
        let mut snapshot = HardwareSnapshot::default();
        let mut packet = [0; 256];
        packet[18] = 70;
        decode_fans(&mut snapshot, &packet);
        assert_eq!(snapshot.fan.cpu_rpm, Some(0.0));
    }

    #[test]
    fn temperature_matches_oem_zero_tdp_branch() {
        for raw in [35, 50, 51, 70, 95, 100] {
            let mut snapshot = HardwareSnapshot::default();
            let mut packet = [0; 256];
            packet[18] = raw;
            decode_fans(&mut snapshot, &packet);
            assert_eq!(snapshot.cpu_temp_c, Some(raw as f64));
        }
    }
    #[test]
    fn fan_packet_counters_are_reported_as_actual_rpm() {
        let mut snapshot = HardwareSnapshot::default();
        let mut packet = [0; 256];
        packet[2..4].copy_from_slice(&474u16.to_be_bytes());
        packet[4..6].copy_from_slice(&480u16.to_be_bytes());
        packet[18] = 60;
        decode_fans(&mut snapshot, &packet);
        assert_eq!(snapshot.fan.cpu_rpm, Some(4549.0));
        assert_eq!(snapshot.fan.gpu_rpm, Some(4492.0));
    }
}
