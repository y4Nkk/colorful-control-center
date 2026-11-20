//! 不依赖 OEM/写通道的系统概览：CPUID、可用物理内存、内存速率与固定磁盘卷。
use serde::Serialize;
use windows_sys::Win32::{
    Storage::FileSystem::{
        GetDiskFreeSpaceExW, GetDriveTypeW, GetLogicalDrives, GetVolumeInformationW,
    },
    System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX},
};
mod cpu;

#[derive(Debug, Serialize)]
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
pub struct Volume {
    pub mount: String,
    pub label: Option<String>,
    pub file_system: Option<String>,
    pub capacity: Option<Capacity>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemOverview {
    pub cpu: CpuIdentity,
    pub memory: Option<Capacity>,
    /// SMBIOS Type 17 已安装内存条的最高配置速率（DDR 有效传输速率，MT/s）。
    pub memory_speed_mts: Option<u32>,
    pub volumes: Vec<Volume>,
    pub warnings: Vec<String>,
}

pub fn read() -> SystemOverview {
    let mut warnings = Vec::new();
    let memory = read_memory()
        .map_err(|e| warnings.push(format!("内存读取失败：{e}")))
        .ok();
    let memory_speed_mts = crate::hal::smbios::read_memory_speed()
        .map_err(|e| warnings.push(format!("内存频率读取失败：{e:#}")))
        .ok();
    let volumes = read_volumes(&mut warnings);
    SystemOverview {
        cpu: cpu::read(&mut warnings),
        memory,
        memory_speed_mts,
        volumes,
        warnings,
    }
}

fn read_memory() -> std::io::Result<Capacity> {
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

fn read_volumes(warnings: &mut Vec<String>) -> Vec<Volume> {
    let mask = unsafe { GetLogicalDrives() };
    if mask == 0 {
        warnings.push(format!("存储读取失败：{}", std::io::Error::last_os_error()));
    }
    (0..26)
        .filter(|index| mask & (1 << index) != 0)
        .filter_map(|index| {
            let letter = b'A' + index as u8;
            let root = [letter as u16, b':' as u16, b'\\' as u16, 0];
            // DRIVE_FIXED = 3；不枚举网络共享、光驱和可移动介质。
            if unsafe { GetDriveTypeW(root.as_ptr()) } != 3 {
                return None;
            }
            let mount = format!("{}:", letter as char);
            let (mut available, mut total) = (0, 0);
            let ok = unsafe {
                GetDiskFreeSpaceExW(
                    root.as_ptr(),
                    &mut available,
                    &mut total,
                    std::ptr::null_mut(),
                )
            };
            let capacity = if ok == 0 {
                warnings.push(format!(
                    "{mount} 容量读取失败：{}",
                    std::io::Error::last_os_error()
                ));
                None
            } else {
                Some(Capacity {
                    total_bytes: total,
                    available_bytes: available,
                })
            };
            let (label, file_system) = match volume_identity(&root) {
                Ok(value) => (Some(value.0), Some(value.1)),
                Err(error) => {
                    warnings.push(format!("{mount} 卷信息读取失败：{error}"));
                    (None, None)
                }
            };
            Some(Volume {
                mount,
                label,
                file_system,
                capacity,
            })
        })
        .collect()
}

fn volume_identity(root: &[u16]) -> std::io::Result<(String, String)> {
    let mut label = [0u16; 261];
    let mut file_system = [0u16; 32];
    if unsafe {
        GetVolumeInformationW(
            root.as_ptr(),
            label.as_mut_ptr(),
            label.len() as u32,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            file_system.as_mut_ptr(),
            file_system.len() as u32,
        )
    } == 0
    {
        return Err(std::io::Error::last_os_error());
    }
    let text = |buffer: &[u16]| {
        String::from_utf16_lossy(
            &buffer[..buffer
                .iter()
                .position(|value| *value == 0)
                .unwrap_or(buffer.len())],
        )
    };
    Ok((text(&label), text(&file_system)))
}
