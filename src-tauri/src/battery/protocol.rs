use super::model::{ChargeState, ChargeValues, Diagnostics};
use anyhow::{Result, bail};

pub fn charge(raw: &[u8; 256]) -> Result<Option<ChargeState>> {
    if raw.iter().all(|v| *v == 0) {
        return Ok(None);
    }
    if raw[0] > 1 {
        bail!("固件返回未知充电保护状态{}", raw[0]);
    }
    let choices = |bytes: &[u8]| -> Result<Vec<u8>> {
        let mut values = Vec::new();
        for value in bytes.iter().copied().filter(|v| *v != 0) {
            if value > 100 {
                bail!("固件充电候选超出百分比范围");
            }
            if !values.contains(&value) {
                values.push(value);
            }
        }
        Ok(values)
    };
    let start_values = choices(&raw[16..32])?;
    let stop_values = choices(&raw[32..48])?;
    if start_values.is_empty() || stop_values.is_empty() {
        bail!("固件没有返回完整的充电阈值候选");
    }
    if raw[1] > 100 || raw[2] > 100 {
        bail!("固件充电阈值超出百分比范围");
    }
    let state = ChargeState {
        current: ChargeValues {
            enabled: raw[0] == 1,
            start: raw[1],
            stop: raw[2],
        },
        start_values,
        stop_values,
    };
    state.validate_snapshot()?;
    Ok(Some(state))
}
pub fn request(values: &ChargeValues) -> [u8; 256] {
    let mut packet = [0; 256];
    packet[..4].copy_from_slice(&[31, u8::from(values.enabled), values.start, values.stop]);
    packet
}
pub fn utility_supported(caps: &[u8]) -> Result<bool> {
    if caps.len() < 15 {
        bail!("原厂电池能力页长度不足");
    }
    match &caps[..2] {
        [1, 0] => Ok(u16::from_le_bytes([caps[13], caps[14]]) & (1 << 9) != 0),
        [0, 0] => Ok(false),
        _ => bail!("未知原厂电池能力页版本"),
    }
}
fn date(raw: u16) -> Option<String> {
    let year = 1980 + (raw >> 9);
    let month = (raw >> 5) & 15;
    let day = raw & 31;
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month {
        2 => {
            if leap {
                29
            } else {
                28
            }
        }
        4 | 6 | 9 | 11 => 30,
        1..=12 => 31,
        _ => return None,
    };
    (day > 0 && day <= days).then(|| format!("{year:04}-{month:02}-{day:02}"))
}
pub fn diagnostics(raw: &[u8; 256]) -> Diagnostics {
    let word = |offset| u16::from_le_bytes([raw[offset], raw[offset + 1]]);
    let dword =
        |offset| u32::from_le_bytes(raw[offset..offset + 4].try_into().expect("固定长度包"));
    let full = word(6);
    let design = word(8);
    Diagnostics {
        version: word(0),
        manufactured_on: date(word(2)),
        cycle_count: word(4),
        full_capacity_raw: full,
        design_capacity_raw: design,
        capacity_pct: (full > 0 && design > 0).then(|| f64::from(full) / f64::from(design) * 100.0),
        battery_status: word(10),
        permanent_failure_status: dword(12),
        operation_status: dword(16),
        stop_charging_threshold: raw[20],
    }
}
