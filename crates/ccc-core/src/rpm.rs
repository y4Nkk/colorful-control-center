//! 原厂十级 RPM 表协议；读首表偏移 0，写首表偏移 16，RPM 均为大端。
use serde::{Deserialize, Serialize};

pub const FAN_IDS: [&str; 4] = ["cpu", "gpu1", "gpu2", "sys"];
pub const LEVELS: usize = 10;
pub const TABLE_SIZE: usize = 112;
pub type Table = [u8; TABLE_SIZE];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpmPoint {
    pub temp_c: u8,
    pub rpm_pct: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pair {
    pub query: u8,
    pub tables: [Table; 2],
}

impl Pair {
    pub fn read(query: u8, packet: &[u8; 256]) -> Result<Self, String> {
        if ![40, 42].contains(&query) {
            return Err("未知 RPM 表读取命令".into());
        }
        Ok(Self {
            query,
            tables: [
                packet[..112].try_into().unwrap(),
                packet[128..240].try_into().unwrap(),
            ],
        })
    }

    pub fn write_packet(&self) -> [u8; 256] {
        let mut packet = [0; 256];
        packet[0] = self.query + 1;
        packet[16..128].copy_from_slice(&self.tables[0]);
        packet[128..240].copy_from_slice(&self.tables[1]);
        packet
    }

    pub fn fan_index(&self, slot: usize) -> usize {
        usize::from(self.query - 40) + slot
    }
}

fn validate_temperatures(points: &[RpmPoint; LEVELS]) -> Result<(), String> {
    if points.iter().any(|p| p.temp_c > 100 || p.rpm_pct > 100)
        || points.windows(2).any(|p| p[0].temp_c >= p[1].temp_c)
    {
        return Err("十级温度须在 0–100°C 严格递增，转速须在 0–100%".into());
    }
    Ok(())
}

pub fn validate_curve(points: &[RpmPoint; LEVELS]) -> Result<(), String> {
    validate_temperatures(points)?;
    if points.iter().any(|p| p.rpm_pct < 20)
        || points.windows(2).any(|p| p[0].rpm_pct > p[1].rpm_pct)
        || points[LEVELS - 1].rpm_pct != 100
    {
        return Err("新曲线转速须在 20–100% 单调不降，末级须为 100%".into());
    }
    Ok(())
}

pub fn maximum_rpm(table: &Table) -> Result<u16, String> {
    let maximum = u16::from_be_bytes([table[10], table[11]]);
    if maximum == 0 {
        return Err("固件未报告最大转速".into());
    }
    Ok(maximum)
}

pub fn decode_curve(table: &Table) -> Result<[RpmPoint; LEVELS], String> {
    let maximum = maximum_rpm(table)?;
    let mut points = [RpmPoint {
        temp_c: 0,
        rpm_pct: 0,
    }; LEVELS];
    for (index, point) in points.iter_mut().enumerate() {
        let at = 62 + index * 2;
        let rpm = u16::from_be_bytes([table[at], table[at + 1]]);
        if rpm > maximum {
            return Err("固件目标 RPM 超过报告的最大转速".into());
        }
        *point = RpmPoint {
            temp_c: table[32 + index],
            rpm_pct: (f64::from(rpm) * 100.0 / f64::from(maximum)).round_ties_even() as u8,
        };
    }
    validate_temperatures(&points)?;
    Ok(points)
}

pub fn encode_curve(
    before: &Table,
    fan_index: usize,
    power_mode: u8,
    points: &[RpmPoint; LEVELS],
) -> Result<Table, String> {
    validate_curve(points)?;
    encode_points(before, fan_index, power_mode, points)
}

/// 原版 Load_default_profile_FanInfo 的固定工厂曲线；不能由用户目标绕过 20% 下限。
pub fn encode_default(before: &Table, fan_index: usize, power_mode: u8) -> Result<Table, String> {
    let points = std::array::from_fn(|i| RpmPoint {
        temp_c: [35, 40, 45, 50, 60, 65, 70, 80, 90, 100][i],
        rpm_pct: [0, 30, 40, 45, 50, 60, 70, 80, 90, 100][i],
    });
    let mut table = encode_points(before, fan_index, power_mode, &points)?;
    table[82..92].fill(26);
    table[92..102].fill(10);
    table[102..112].fill(0x61);
    Ok(table)
}

fn encode_points(
    before: &Table,
    fan_index: usize,
    power_mode: u8,
    points: &[RpmPoint; LEVELS],
) -> Result<Table, String> {
    if fan_index >= FAN_IDS.len() || power_mode > 4 {
        return Err("未知风扇或电源模式".into());
    }
    let maximum = maximum_rpm(before)?;
    let profile = if power_mode == 4 { 0 } else { power_mode };
    let mut table = *before;
    table[0] = 128 + fan_index as u8 * 16 + profile;
    for (index, point) in points.iter().enumerate() {
        table[22 + index] = if index == 0 {
            0
        } else {
            points[index - 1].temp_c + 1
        };
        table[32 + index] = point.temp_c;
        let rpm = (f64::from(maximum) * f64::from(point.rpm_pct) / 100.0).round_ties_even() as u16;
        for at in [42 + index * 2, 62 + index * 2] {
            table[at..at + 2].copy_from_slice(&rpm.to_be_bytes());
        }
    }
    Ok(table)
}

#[cfg(test)]
mod tests;
