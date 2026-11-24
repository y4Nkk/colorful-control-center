//! HAL 只拥有硬件传输、采样和本轮写入前的恢复快照。
#[cfg(not(test))]
pub(crate) mod airplane;
#[cfg(not(test))]
pub(crate) mod audio;
pub(crate) mod authorization;
pub(crate) mod beep_audio;
mod cpu_utility;
pub(crate) mod event_log;
mod fan_guard;
mod fan_safety;
pub(crate) use fan_safety::RECORD_NAME as SOFTWARE_FAN_RECORD_NAME;
pub mod firmware;
pub mod key_events;
pub mod key_filter;
mod keyboard_inventory;
mod memory;
mod monitor;
#[cfg(test)]
mod monitor_tests;
pub mod msr;
pub mod nvapi;
pub mod nvidia;
pub mod oem_dchu;
pub(crate) mod oem_identity;
pub(crate) mod oem_owner;
pub mod overview;
pub(crate) mod panel_aux;
pub(crate) mod panel_inventory;
#[cfg(not(test))]
pub mod pawnio;
pub(crate) mod power_events;
pub mod power_plan;
pub(crate) mod screen_brightness;
pub(crate) mod smbios;
pub(crate) mod system_action;
#[cfg(not(test))]
pub(crate) mod touchpad;
#[cfg(not(test))]
mod wmi;

pub use cpu_utility::Reading as CpuUtilityReading;
pub use memory::OemMemoryReading;

use anyhow::{Result, anyhow};
use ccc_core::{fan_sensors::FanReadings, settings::Patch};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    sync::{
        Mutex, MutexGuard,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Clone, Serialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct NvidiaSnapshot {
    pub name: String,
    pub temp_c: Option<f64>,
    pub fan_pct: Option<f64>,
    pub utilization_pct: Option<f64>,
    pub mem_used_mb: Option<f64>,
    pub mem_total_mb: Option<f64>,
    pub graphics_clock_mhz: Option<u32>,
    pub memory_clock_mhz: Option<u32>,
    pub power_draw_w: Option<f64>,
    pub power_limit_w: Option<f64>,
    pub cuda_cores: Option<u32>,
    pub memory_bus_width_bits: Option<u32>,
}

#[derive(Clone, Serialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct HardwareSnapshot {
    pub dchu_available: bool,
    pub oem_power_mode: Option<u8>,
    pub cpu_utility: Option<CpuUtilityReading>,
    pub fan: FanReadings,
    pub nvidia: Option<NvidiaSnapshot>,
    pub oem_memory: Option<OemMemoryReading>,
    pub nvidia_suspended: bool,
    pub sampled_at_ms: u64,
    pub warnings: Vec<String>,
}

#[derive(Default)]
pub struct Recovery {
    pub oem_mode: Option<crate::powerctl::mode::Snapshot>,
    pub power: Vec<Patch>,
    pub runtime_pl: Option<u64>,
    pub voltage: BTreeMap<u64, i16>,
    pub fan: Option<crate::fanctl::FanCheckpoint>,
}

pub struct Hal {
    calibration_record: Option<std::path::PathBuf>,
    automatic_mode_record: Option<std::path::PathBuf>,
    cpu_utility: Mutex<cpu_utility::Sampler>,
    #[cfg(test)]
    pub(crate) test_manufacturer: Option<String>,
    #[cfg(test)]
    pub(crate) test_usb_keyboard: Option<std::sync::Arc<dyn Fn() -> Result<bool> + Send + Sync>>,
    #[cfg(test)]
    pub(crate) test_screen_brightness:
        Option<std::sync::Arc<dyn Fn() -> Result<screen_brightness::Reading> + Send + Sync>>,
    #[cfg(test)]
    pub(crate) test_panel_inventory:
        Option<std::sync::Arc<dyn Fn() -> Result<panel_inventory::Inventory> + Send + Sync>>,
    pub(crate) system_power: std::sync::Arc<dyn power_plan::PowerAccess>,
    #[cfg(test)]
    test_msr: Option<std::sync::Arc<dyn ccc_core::msr::MsrAccess + Send + Sync>>,
    #[cfg(test)]
    pub(crate) test_dtt_driver: Option<std::sync::Arc<dyn Fn() -> Result<bool> + Send + Sync>>,
    #[cfg(test)]
    pub(crate) test_nvidia:
        Option<std::sync::Arc<dyn Fn() -> Result<Option<NvidiaSnapshot>> + Send + Sync>>,
    dchu: Mutex<Option<oem_dchu::Dchu>>,
    nvidia: Mutex<Option<nvidia::Nvidia>>,
    nvidia_suspended: AtomicBool,
    recovery: Mutex<Recovery>,
    fan_guard: Mutex<Option<crate::watchdog::Watchdog>>,
    #[cfg(test)]
    pub(crate) test_fan_guard:
        Option<std::sync::Arc<dyn Fn() -> Result<crate::watchdog::Watchdog> + Send + Sync>>,
    #[cfg(test)]
    pub(crate) test_fan_control: Option<firmware::ReadDirectControl>,
    safety: Mutex<String>,
    mutation: std::sync::Arc<Mutex<()>>,
    fan_recovery_needed: AtomicBool,
    software_fan_safety: Mutex<fan_safety::journal::Journal>,
    pub(crate) software_fan: Mutex<Option<crate::fanctl::software::Session>>,
    pub(crate) software_progress: Mutex<Option<std::time::Instant>>,
    pub(crate) reviewed_dsdt: bool,
}

impl Hal {
    pub fn new() -> Self {
        let dchu = oem_dchu::Dchu::load()
            .map_err(|e| log::warn!("[hal] DCHU: {e:#}"))
            .ok();
        Self {
            calibration_record: None,
            automatic_mode_record: None,
            cpu_utility: Mutex::new(cpu_utility::Sampler::default()),
            #[cfg(test)]
            test_manufacturer: None,
            #[cfg(test)]
            test_usb_keyboard: None,
            #[cfg(test)]
            test_screen_brightness: None,
            #[cfg(test)]
            test_panel_inventory: None,
            system_power: std::sync::Arc::new(power_plan::NativePower),
            #[cfg(test)]
            test_msr: None,
            #[cfg(test)]
            test_dtt_driver: None,
            #[cfg(test)]
            test_nvidia: None,
            dchu: Mutex::new(dchu),
            nvidia: Mutex::new(None),
            nvidia_suspended: AtomicBool::new(false),
            recovery: Mutex::new(Recovery::default()),
            mutation: std::sync::Arc::new(Mutex::new(())),
            fan_guard: Mutex::new(None),
            #[cfg(test)]
            test_fan_guard: None,
            #[cfg(test)]
            test_fan_control: None,
            safety: Mutex::new("固件控制".into()),
            fan_recovery_needed: AtomicBool::new(false),
            software_fan_safety: Mutex::new(fan_safety::journal::Journal::default()),
            software_fan: Mutex::new(None),
            software_progress: Mutex::new(None),
            reviewed_dsdt: firmware::reviewed_dsdt(),
        }
    }

    pub fn begin_write(&self) -> Result<MutexGuard<'_, ()>> {
        self.mutation
            .lock()
            .map_err(|_| anyhow!("控制事务异常，请重启应用后重试"))
    }
    #[cfg(test)]
    pub(crate) fn test_write_gate(&self) -> std::sync::Arc<Mutex<()>> {
        self.mutation.clone()
    }
    #[cfg(test)]
    pub(crate) fn test_dchu_available(&self) -> bool {
        self.dchu.try_lock().is_ok()
    }
    pub(crate) fn bind_calibration_record(&mut self, path: std::path::PathBuf) {
        self.calibration_record = Some(path);
    }
    pub(crate) fn bind_automatic_mode_record(&mut self, path: std::path::PathBuf) {
        self.automatic_mode_record = Some(path);
    }
    pub(crate) fn ensure_automatic_profile_allowed(&self) -> Result<()> {
        if let Some(path) = &self.automatic_mode_record {
            crate::powerctl::mode::supply::ensure_inactive(path)?;
        }
        Ok(())
    }
    pub(crate) fn ensure_mode_change_allowed(&self) -> Result<()> {
        if let Some(path) = &self.calibration_record {
            crate::battery::calibration::ensure_system_plan_available(path)?;
        }
        Ok(())
    }

    pub(crate) fn control_gate(&self) -> std::sync::Arc<Mutex<()>> {
        self.mutation.clone()
    }

    #[cfg(test)]
    #[allow(dead_code)] // 隔离集成测试复用；原生 lib 测试不构造此实例。
    pub(crate) fn test_instance(
        dchu: oem_dchu::Dchu,
        guard: Option<crate::watchdog::Watchdog>,
    ) -> Self {
        Self {
            calibration_record: None,
            automatic_mode_record: None,
            cpu_utility: Mutex::new(cpu_utility::Sampler::default()),
            #[cfg(test)]
            test_manufacturer: None,
            #[cfg(test)]
            test_usb_keyboard: None,
            test_screen_brightness: None,
            test_panel_inventory: None,
            system_power: std::sync::Arc::new(power_plan::UnavailablePower),
            test_msr: None,
            test_dtt_driver: None,
            test_nvidia: None,
            dchu: Mutex::new(Some(dchu)),
            nvidia: Mutex::new(None),
            nvidia_suspended: AtomicBool::new(false),
            recovery: Mutex::new(Recovery::default()),
            fan_guard: Mutex::new(guard),
            test_fan_guard: None,
            test_fan_control: None,
            safety: Mutex::new("固件控制".into()),
            mutation: std::sync::Arc::new(Mutex::new(())),
            fan_recovery_needed: AtomicBool::new(false),
            software_fan_safety: Mutex::new(fan_safety::journal::Journal::default()),
            software_fan: Mutex::new(None),
            software_progress: Mutex::new(None),
            reviewed_dsdt: true,
        }
    }

    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) fn with_test_msr(
        mut self,
        io: std::sync::Arc<dyn ccc_core::msr::MsrAccess + Send + Sync>,
    ) -> Self {
        self.test_msr = Some(io);
        self
    }
    #[cfg(test)]
    pub(crate) fn set_test_dtt_driver(
        &mut self,
        provider: std::sync::Arc<dyn Fn() -> Result<bool> + Send + Sync>,
    ) {
        self.test_dtt_driver = Some(provider);
    }

    pub(crate) fn open_msr(&self) -> Result<msr::MsrIo> {
        msr::MsrIo::open(self)
    }

    pub(crate) fn request_fan_recovery(&self) -> Result<()> {
        self.fan_recovery_needed.store(true, Ordering::SeqCst);
        self.software_progress
            .lock()
            .map_err(|_| anyhow!("软件曲线进度异常"))?
            .take();
        if let Some(client) = self
            .fan_guard
            .lock()
            .map_err(|_| anyhow!("风扇监护状态异常"))?
            .as_mut()
        {
            client.stop_heartbeats();
        }
        Ok(())
    }

    pub fn recovery(&self) -> Result<MutexGuard<'_, Recovery>> {
        self.recovery.lock().map_err(|_| anyhow!("恢复快照不可用"))
    }

    pub fn with_dchu<T>(&self, f: impl FnOnce(&oem_dchu::Dchu) -> Result<T>) -> Result<T> {
        let guard = self.dchu.lock().map_err(|_| anyhow!("DCHU 通道状态异常"))?;
        #[cfg(not(test))]
        let mut guard = guard;
        #[cfg(not(test))]
        if guard.is_none() {
            *guard = Some(oem_dchu::Dchu::load()?);
        }
        let dchu = guard.as_ref().ok_or_else(|| anyhow!("DCHU 通道不可用"))?;
        f(dchu)
    }

    fn sample_nvidia(&self) -> Result<Option<NvidiaSnapshot>> {
        let mut guard = self
            .nvidia
            .lock()
            .map_err(|_| anyhow!("GPU 传感连接状态异常"))?;
        if self.nvidia_suspended.load(Ordering::SeqCst) {
            return Ok(None);
        }
        #[cfg(test)]
        if let Some(source) = &self.test_nvidia {
            return source();
        }
        #[cfg(not(test))]
        if guard.is_none() {
            *guard = Some(nvidia::Nvidia::init()?);
        }
        let result = guard.as_ref().map(nvidia::Nvidia::sample).transpose();
        if result.is_err() {
            guard.take();
        }
        result
    }
    pub(crate) fn gpu_suspended(&self) -> bool {
        self.nvidia_suspended.load(Ordering::SeqCst)
    }
    pub(crate) fn suspend_gpu(&self, suspended: bool) -> Result<()> {
        let mut guard = self
            .nvidia
            .lock()
            .map_err(|_| anyhow!("GPU传感连接状态异常"))?;
        if self.nvidia_suspended.swap(suspended, Ordering::SeqCst) != suspended {
            guard.take();
        }
        Ok(())
    }
    pub(crate) fn set_safety_status(&self, value: String) -> Result<()> {
        *self.safety.lock().map_err(|_| anyhow!("监护状态不可用"))? = value;
        Ok(())
    }
}
