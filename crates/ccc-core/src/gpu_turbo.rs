//! FnKey 7.19.5.0 的 GPU Turbo 目标计算，不访问驱动或固件。
use serde::{Deserialize, Serialize};
pub mod mode;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Capabilities {
    pub limit_clock: bool,
    pub all_mode_oc: bool,
}

impl Capabilities {
    pub fn needs_wmi96(packet: &[u8; 256]) -> Result<bool, String> {
        match (packet[0], packet[1]) {
            (0, 0) => Ok(true),
            (1, 0) => {
                let a = u32::from_le_bytes(packet[2..6].try_into().unwrap());
                let b = u32::from_le_bytes(packet[6..10].try_into().unwrap());
                let c = u16::from_le_bytes(packet[10..12].try_into().unwrap()) as u32;
                let d = u16::from_le_bytes(packet[13..15].try_into().unwrap()) as u32;
                Ok(a.wrapping_add(b).wrapping_add(c).wrapping_add(d) == 0)
            }
            _ => Err("未知原厂 GPU 能力包版本".into()),
        }
    }
    /// v0 及 v1 空特征包由调用者提供 WMI 96 的有效读数。
    pub fn from_packet(packet: &[u8; 256], wmi96: Option<i32>) -> Result<Self, String> {
        let needs_wmi96 = Self::needs_wmi96(packet)?;
        let (limit_clock, raw96) = match (packet[0], packet[1]) {
            (0, 0) => (packet[13] & 1 != 0, wmi96),
            (1, 0) => {
                let c = u16::from_le_bytes(packet[10..12].try_into().unwrap()) as u32;
                (
                    packet[15] & 1 != 0,
                    if needs_wmi96 { wmi96 } else { Some(c as i32) },
                )
            }
            _ => unreachable!("能力版本已在 needs_wmi96 校验"),
        };
        let raw96 = raw96.ok_or("原厂 WMI 96 能力不可读")?;
        Ok(Self {
            limit_clock,
            all_mode_oc: raw96 & 0x20 != 0,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Limits {
    pub entertainment_mhz: u16,
    pub performance_mhz: u16,
    pub saving_mhz: u16,
    pub quiet_mhz: u16,
    pub thermal_mhz: u16,
}
impl Limits {
    pub fn from_packet(packet: &[u8; 256]) -> Self {
        let word = |offset: usize| u16::from_le_bytes([packet[offset], packet[offset + 1]]);
        Self {
            entertainment_mhz: word(2),
            performance_mhz: word(4),
            saving_mhz: word(6),
            quiet_mhz: word(8),
            thermal_mhz: word(10),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Offsets {
    pub core_mhz: i32,
    pub memory_mhz: i32,
}
impl Offsets {
    fn decode(bytes: &[u8]) -> i32 {
        let value = u16::from_le_bytes([bytes[0], bytes[1]]) as i32;
        if value & 0x8000 != 0 {
            value - 65535
        } else {
            value
        }
    }
    pub fn saved(bytes: &[u8]) -> Result<[Self; 2], String> {
        if bytes.len() != 8 {
            return Err("原厂 GPU 偏移保存值长度无效".into());
        }
        Ok(std::array::from_fn(|i| Self {
            core_mhz: Self::decode(&bytes[i * 4..]),
            memory_mhz: Self::decode(&bytes[i * 4 + 2..]),
        }))
    }
    const ZERO: Self = Self {
        core_mhz: 0,
        memory_mhz: 0,
    };
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Target {
    /// None 表示不触碰锁频；Some(0) 表示按原厂请求解除锁频。
    pub lock_mhz: Option<u32>,
    /// None 表示不触碰偏移；有值时按原厂最多操作前两张显卡。
    pub offsets: Option<Vec<Offsets>>,
}

pub fn target(
    event: u8,
    mode: u8,
    on_ac: Option<bool>,
    caps: Capabilities,
    limits: Limits,
    saved: [Offsets; 2],
    gpu_count: usize,
    base_mhz: Option<u32>,
) -> Result<Target, String> {
    if gpu_count == 0 {
        return Err("没有可用的 NVIDIA 显卡".into());
    }
    if event == 96 {
        let lock = if limits.thermal_mhz == 0 {
            base_mhz
                .filter(|mhz| *mhz > 0)
                .ok_or("原厂基础图形频率不可读")?
        } else {
            u32::from(limits.thermal_mhz)
        };
        return Ok(Target {
            lock_mhz: Some(lock),
            offsets: None,
        });
    }
    if event != 97 {
        return Err("未知原厂 GPU Turbo 事件".into());
    }
    let (lock, saved_oc) = match mode {
        0 => (Some(limits.quiet_mhz), caps.all_mode_oc),
        1 => (Some(limits.saving_mhz), caps.all_mode_oc),
        2 if on_ac.ok_or("当前供电状态不可读")? => (Some(limits.performance_mhz), true),
        2 => (None, false),
        3 => (Some(limits.entertainment_mhz), caps.all_mode_oc),
        _ => return Err("当前模式不在原厂 GPU Turbo 处理范围".into()),
    };
    Ok(Target {
        lock_mhz: lock.map(|mhz| if caps.limit_clock { u32::from(mhz) } else { 0 }),
        offsets: Some(
            (0..gpu_count.min(2))
                .map(|i| if saved_oc { saved[i] } else { Offsets::ZERO })
                .collect(),
        ),
    })
}

#[cfg(test)]
mod tests;
