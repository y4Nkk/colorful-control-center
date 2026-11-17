use super::oem_dchu::Dchu;
use serde::Serialize;

#[derive(Clone, Serialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct OemMemoryReading {
    pub reported_clock_mhz: Option<u32>,
    pub reference_clock_mhz: Option<u32>,
}

pub(super) fn read(io: &Dchu, warnings: &mut Vec<String>) -> Option<OemMemoryReading> {
    let reported_clock_mhz = match io.read_settings(6, 23, 4) {
        Ok(bytes) => {
            let value = i32::from_le_bytes(bytes.try_into().unwrap());
            if value < 0 {
                warnings.push(format!("内存原厂报告频率无效：{value}"));
            }
            (value > 0).then_some(value as u32)
        }
        Err(error) => {
            warnings.push(format!("内存原厂报告频率读取失败：{error:#}"));
            None
        }
    };
    let reference_clock_mhz = match io.read_settings(6, 230, 2) {
        Ok(bytes) => {
            let value = u16::from_le_bytes(bytes.try_into().unwrap());
            (value > 0).then_some(value as u32)
        }
        Err(error) => {
            warnings.push(format!("内存原厂参考频率读取失败：{error:#}"));
            None
        }
    };
    (reported_clock_mhz.is_some() || reference_clock_mhz.is_some()).then_some(OemMemoryReading {
        reported_clock_mhz,
        reference_clock_mhz,
    })
}

#[cfg(test)]
#[path = "memory_tests.rs"]
mod tests;
