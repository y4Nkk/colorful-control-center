//! GPU 传感器（T4：NVML，用户态，无内核依赖）。

use anyhow::Result;
use nvml_wrapper::Nvml;

use super::NvidiaSnapshot;

pub struct Nvidia {
    nvml: Nvml,
}

unsafe impl Send for Nvidia {}
unsafe impl Sync for Nvidia {}

impl Nvidia {
    pub fn init() -> Result<Self> {
        // nvml-wrapper 默认按已安装驱动查找 nvml.dll
        Ok(Self { nvml: Nvml::init()? })
    }

    pub fn sample(&self) -> NvidiaSnapshot {
        let mut out = NvidiaSnapshot::default();
        let Ok(device) = self.nvml.device_by_index(0) else {
            return out;
        };
        if let Ok(name) = device.name() {
            out.name = name;
        }
        if let Ok(temp) = device.temperature(nvml_wrapper::enum_wrappers::device::TemperatureSensor::Gpu) {
            out.temp_c = Some(temp as f64);
        }
        // 笔记本 dGPU 风扇通常不归 NVML 管（Err 属预期）
        if let Ok(fan) = device.fan_speed(0) {
            out.fan_pct = Some(fan as f64);
        }
        if let Ok(util) = device.utilization_rates() {
            out.utilization_pct = Some(util.gpu as f64);
        }
        if let Ok(mem) = device.memory_info() {
            out.mem_used_mb = Some(mem.used as f64 / 1024.0 / 1024.0);
            out.mem_total_mb = Some(mem.total as f64 / 1024.0 / 1024.0);
        }
        out
    }
}
