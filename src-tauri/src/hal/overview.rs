//! Windows 系统用量与设备资料，各自按变化频率读取。
use super::smbios::memory::{self, MemoryInventory, MemoryModule, MemorySlots};
use anyhow::{Result, anyhow};
use serde::Serialize;
use std::sync::Mutex;
use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
mod cpu;
mod volumes;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CpuIdentity {
    pub vendor: &'static str,
    pub name: Option<String>,
    pub physical_cores: Option<u32>,
    pub logical_processors: Option<u32>,
    pub base_clock_mhz: Option<u32>,
    pub max_clock_mhz: Option<u32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Capacity {
    pub total_bytes: u64,
    pub available_bytes: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemInventory {
    pub cpu: CpuIdentity,
    /// SMBIOS Type 17 已安装内存条共同报告的配置速率（MT/s）。
    pub memory_speed_mts: Option<u32>,
    pub(crate) memory_modules: Option<Vec<MemoryModule>>,
    pub(crate) memory_slots: Option<MemorySlots>,
    pub volumes: Option<Vec<volumes::Volume>>,
    pub warnings: Vec<String>,
}

struct InventoryReader {
    cpu: Option<CpuIdentity>,
    memory: Option<MemoryInventory>,
    volumes: volumes::MetadataCache,
}

static INVENTORY: Mutex<InventoryReader> = Mutex::new(InventoryReader {
    cpu: None,
    memory: None,
    volumes: volumes::MetadataCache::new(),
});

pub(crate) fn read_inventory(refresh_volume_metadata: bool) -> Result<SystemInventory> {
    let mut reader = INVENTORY
        .lock()
        .map_err(|_| anyhow!("系统设备资料状态异常"))?;
    Ok(reader.read(refresh_volume_metadata))
}

impl InventoryReader {
    fn read(&mut self, refresh_volume_metadata: bool) -> SystemInventory {
        let mut warnings = Vec::new();
        let cpu = self.cpu.clone().unwrap_or_else(|| {
            let identity = cpu::read(&mut warnings);
            if warnings.is_empty() {
                self.cpu = Some(identity.clone());
            }
            identity
        });
        if self.memory.is_none() {
            match memory::read_inventory() {
                Ok(inventory) => self.memory = Some(inventory),
                Err(error) => warnings.push(format!("内存设备资料读取失败：{error:#}")),
            }
        }
        let volumes = self
            .volumes
            .read(refresh_volume_metadata, &mut warnings)
            .map_err(|error| warnings.push(format!("存储读取失败：{error}")))
            .ok();
        SystemInventory {
            cpu,
            memory_speed_mts: self.memory.as_ref().and_then(|memory| memory.speed_mts),
            memory_modules: self.memory.as_ref().map(|memory| memory.modules.clone()),
            memory_slots: self.memory.as_ref().and_then(|memory| memory.slots),
            volumes,
            warnings,
        }
    }
}

pub(crate) fn read_memory() -> std::io::Result<Capacity> {
    let mut memory = MEMORYSTATUSEX {
        dwLength: size_of::<MEMORYSTATUSEX>() as u32,
        ..Default::default()
    };
    if unsafe { GlobalMemoryStatusEx(&mut memory) } == 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(Capacity {
        total_bytes: memory.ullTotalPhys,
        available_bytes: memory.ullAvailPhys,
    })
}
