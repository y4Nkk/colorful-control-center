use super::CpuIdentity;
use std::io::{Error, ErrorKind, Result};
use windows_sys::Win32::{
    Foundation::ERROR_INSUFFICIENT_BUFFER,
    System::{
        SystemInformation::{GetLogicalProcessorInformationEx, RelationProcessorCore},
        Threading::GetActiveProcessorCount,
    },
};

pub(super) fn read(warnings: &mut Vec<String>) -> CpuIdentity {
    let mut cpu = CpuIdentity {
        vendor: "unknown",
        name: None,
        physical_cores: None,
        logical_processors: None,
        base_clock_mhz: None,
        max_clock_mhz: None,
    };
    identify(&mut cpu);
    match physical_cores() {
        Ok(count) => cpu.physical_cores = Some(count),
        Err(error) => warnings.push(format!("CPU 核心数读取失败：{error}")),
    }
    let logical = unsafe { GetActiveProcessorCount(u16::MAX) };
    if logical > 0 {
        cpu.logical_processors = Some(logical);
    } else {
        warnings.push(format!(
            "CPU 逻辑处理器数读取失败：{}",
            Error::last_os_error()
        ));
    }
    cpu
}

fn identify(cpu: &mut CpuIdentity) {
    #[cfg(target_arch = "x86_64")]
    {
        use std::arch::x86_64::__cpuid;
        let id = __cpuid(0);
        let vendor = [
            id.ebx.to_le_bytes(),
            id.edx.to_le_bytes(),
            id.ecx.to_le_bytes(),
        ]
        .concat();
        cpu.vendor = match vendor.as_slice() {
            b"GenuineIntel" => "intel",
            b"AuthenticAMD" => "amd",
            _ => "unknown",
        };
        let mut bytes = Vec::new();
        if __cpuid(0x8000_0000).eax >= 0x8000_0004 {
            for leaf in 0x8000_0002..=0x8000_0004 {
                let info = __cpuid(leaf);
                for register in [info.eax, info.ebx, info.ecx, info.edx] {
                    bytes.extend_from_slice(&register.to_le_bytes());
                }
            }
        }
        let name = String::from_utf8_lossy(&bytes)
            .trim_matches('\0')
            .trim()
            .to_string();
        cpu.name = (!name.is_empty()).then_some(name);
        if cpu.vendor == "intel" && id.eax >= 0x16 {
            let frequency = __cpuid(0x16);
            cpu.base_clock_mhz = declared_frequency(frequency.eax);
            cpu.max_clock_mhz = declared_frequency(frequency.ebx);
        }
    }
}

fn declared_frequency(value: u32) -> Option<u32> {
    let mhz = value & 0xffff;
    (mhz > 0).then_some(mhz)
}

fn physical_cores() -> Result<u32> {
    let mut length = 0;
    unsafe {
        GetLogicalProcessorInformationEx(RelationProcessorCore, std::ptr::null_mut(), &mut length);
    }
    let error = Error::last_os_error();
    if error.raw_os_error() != Some(ERROR_INSUFFICIENT_BUFFER as i32) || length == 0 {
        return Err(error);
    }
    // API 的变长记录按 Size 行进；u64 存储保证 Windows 结构的对齐。
    let mut buffer = vec![0u64; (length as usize).div_ceil(8)];
    if unsafe {
        GetLogicalProcessorInformationEx(
            RelationProcessorCore,
            buffer.as_mut_ptr().cast(),
            &mut length,
        )
    } == 0
    {
        return Err(Error::last_os_error());
    }
    if length as usize > buffer.len() * 8 {
        return Err(Error::new(ErrorKind::InvalidData, "CPU 拓扑长度无效"));
    }
    let bytes =
        unsafe { std::slice::from_raw_parts(buffer.as_ptr().cast::<u8>(), length as usize) };
    count_core_records(bytes)
}

fn count_core_records(mut bytes: &[u8]) -> Result<u32> {
    let mut count = 0;
    while !bytes.is_empty() {
        if bytes.len() < 8 {
            return Err(Error::new(ErrorKind::InvalidData, "CPU 拓扑记录不完整"));
        }
        let relationship = i32::from_le_bytes(bytes[..4].try_into().unwrap());
        let size = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
        if relationship != RelationProcessorCore || size < 8 || size > bytes.len() {
            return Err(Error::new(ErrorKind::InvalidData, "CPU 拓扑记录无效"));
        }
        count += 1;
        bytes = &bytes[size..];
    }
    if count == 0 {
        return Err(Error::new(ErrorKind::InvalidData, "CPU 拓扑没有活动核心"));
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn record(size: u32) -> Vec<u8> {
        let mut bytes = vec![0; size as usize];
        bytes[..4].copy_from_slice(&RelationProcessorCore.to_le_bytes());
        bytes[4..8].copy_from_slice(&size.to_le_bytes());
        bytes
    }
    #[test]
    fn counts_variable_sized_core_records() {
        assert_eq!(
            count_core_records(&[record(48), record(64), record(48)].concat()).unwrap(),
            3
        );
    }
    #[test]
    fn rejects_truncated_and_invalid_topology() {
        for bytes in [
            vec![],
            vec![0; 7],
            vec![0; 8],
            [record(48), vec![0; 4]].concat(),
        ] {
            assert!(count_core_records(&bytes).is_err());
        }
        let mut bytes = record(48);
        bytes[4..8].copy_from_slice(&100u32.to_le_bytes());
        assert!(count_core_records(&bytes).is_err());
        let mut bytes = record(48);
        bytes[..4].copy_from_slice(&3i32.to_le_bytes());
        assert!(count_core_records(&bytes).is_err());
    }
    #[test]
    fn missing_frequency_stays_unknown_and_reserved_bits_are_ignored() {
        assert_eq!(declared_frequency(0), None);
        assert_eq!(declared_frequency(0xffff_0000), None);
        assert_eq!(declared_frequency(0xffff_0960), Some(2400));
    }
    #[test]
    fn parameter_contract_keeps_unknowns_and_units_separate() {
        let cpu = CpuIdentity {
            vendor: "intel",
            name: Some("Fixture CPU".into()),
            physical_cores: Some(14),
            logical_processors: Some(20),
            base_clock_mhz: Some(2400),
            max_clock_mhz: None,
        };
        let cpu = serde_json::to_value(cpu).unwrap();
        assert_eq!(cpu["physicalCores"], 14);
        assert_eq!(cpu["logicalProcessors"], 20);
        assert_eq!(cpu["baseClockMhz"], 2400);
        assert!(cpu["maxClockMhz"].is_null());
        let gpu = crate::hal::NvidiaSnapshot {
            graphics_clock_mhz: Some(1900),
            memory_clock_mhz: Some(8000),
            power_draw_w: Some(42.5),
            power_limit_w: Some(115.0),
            ..Default::default()
        };
        let gpu = serde_json::to_value(gpu).unwrap();
        assert_eq!(gpu["graphicsClockMhz"], 1900);
        assert_eq!(gpu["memoryClockMhz"], 8000);
        assert_eq!(gpu["powerDrawW"], 42.5);
        assert_eq!(gpu["powerLimitW"], 115.0);
        assert!(gpu["cudaCores"].is_null());
        assert!(gpu["memoryBusWidthBits"].is_null());
    }
}
