//! 原厂刷新率策略的模式、供电和候选规则；不访问设备或执行切换。

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Firmware {
    pub supported: bool,
    pub default_enabled: bool,
}
impl Firmware {
    pub fn from_packet(packet: &[u8; 256]) -> Result<Self, &'static str> {
        match [packet[0], packet[1]] {
            [0, 0] => Ok(Self {
                supported: true,
                default_enabled: true,
            }),
            [1, 0] => Ok(Self {
                supported: packet[18] & 0x20 == 0,
                default_enabled: packet[18] & 0x80 == 0,
            }),
            _ => Err("原厂刷新率能力版本未知"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trigger {
    ModeChange,
    SupplyChange,
    Startup,
    Resume,
    UserChange,
    DcModeChange,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    None,
    Low,
    SaveThenLow,
    RestoreSaved,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Facts {
    pub customer: u8,
    pub mode: u8,
    pub previous_mode: Option<u8>,
    pub on_ac: Option<bool>,
    pub supported: bool,
    pub checked: bool,
    pub trigger: Trigger,
}
pub fn decide(facts: Facts) -> Result<Decision, &'static str> {
    if !facts.supported || facts.trigger == Trigger::Resume {
        return Ok(Decision::None);
    }
    if facts.mode > 4 || facts.previous_mode.is_some_and(|mode| mode > 4) {
        return Err("原厂刷新率策略模式未知");
    }
    if facts.customer == 5 {
        return Ok(epson(facts));
    }
    ordinary(facts)
}
fn epson(facts: Facts) -> Decision {
    use Decision::*;
    use Trigger::*;
    let eligible = matches!(
        facts.trigger,
        ModeChange | Startup | UserChange | DcModeChange
    );
    if !eligible {
        return None;
    }
    if !facts.checked {
        return if matches!(facts.mode, 0 | 1) {
            RestoreSaved
        } else {
            None
        };
    }
    match facts.mode {
        0 | 1
            if matches!(facts.trigger, ModeChange | DcModeChange)
                && matches!(facts.previous_mode, Some(2 | 3)) =>
        {
            SaveThenLow
        }
        0 | 1 => Low,
        2 | 3 if matches!(facts.previous_mode, Some(0 | 1)) => RestoreSaved,
        _ => None,
    }
}
fn ordinary(facts: Facts) -> Result<Decision, &'static str> {
    use Decision::*;
    use Trigger::*;
    let dc_mode = facts.trigger == DcModeChange;
    let regular = matches!(
        facts.trigger,
        ModeChange | SupplyChange | Startup | UserChange
    );
    if !dc_mode && !regular {
        return Ok(None);
    }
    if matches!(facts.mode, 0 | 2 | 3) {
        if facts.checked && matches!(facts.previous_mode, Some(1 | 4)) {
            return if dc_mode || !facts.on_ac.ok_or("供电状态未知")? {
                Ok(RestoreSaved)
            } else {
                Ok(None)
            };
        }
        return Ok(None);
    }
    if !facts.checked {
        return if dc_mode || !facts.on_ac.ok_or("供电状态未知")? {
            Ok(RestoreSaved)
        } else {
            Ok(None)
        };
    }
    if facts.on_ac.ok_or("供电状态未知")? {
        Ok(if dc_mode || facts.previous_mode.is_none() {
            RestoreSaved
        } else {
            None
        })
    } else {
        Ok(SaveThenLow)
    }
}

pub fn candidates(frequencies: &[u32]) -> Vec<u32> {
    let mut result: Vec<_> = frequencies
        .iter()
        .copied()
        .filter(|frequency| *frequency >= 40 && *frequency != 48)
        .collect();
    result.sort_unstable();
    result.dedup();
    result
}
pub fn lowest(frequencies: &[u32]) -> Result<u32, &'static str> {
    candidates(frequencies)
        .first()
        .copied()
        .ok_or("没有原厂支持的刷新率")
}
pub fn saved_or_highest(frequencies: &[u32], saved: u8) -> Result<u32, &'static str> {
    let rates = candidates(frequencies);
    let high = rates.last().copied().ok_or("没有原厂支持的刷新率")?;
    Ok(if rates.contains(&u32::from(saved)) {
        u32::from(saved)
    } else {
        high
    })
}
pub fn original_saved_byte(frequency: u32) -> u8 {
    frequency as u8
}

#[cfg(test)]
#[path = "oem_refresh_tests.rs"]
mod tests;
