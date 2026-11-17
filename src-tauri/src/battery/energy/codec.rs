use super::model::*;
use anyhow::{Result, bail};

pub fn supported(page: &[u8]) -> Result<bool> {
    if page.len() < 15 {
        bail!("节能能力页长度不足");
    }
    if page[..2] == [0, 0] {
        return Ok(false);
    }
    if page[..2] != [1, 0] {
        bail!("未确认节能能力页版本");
    }
    Ok(u16::from_le_bytes([page[13], page[14]]) & (1 << 8) != 0)
}
pub fn capability(page: &[u8], defaults: &[u8; 256]) -> Result<Option<Capability>> {
    if !supported(page)? {
        return Ok(None);
    }
    let defaulted = defaults[208] == 0 || defaults[209] == 0;
    let factory = if defaulted {
        Limits {
            floor: 95,
            ceiling: 100,
        }
    } else {
        Limits {
            floor: defaults[209],
            ceiling: defaults[208],
        }
    };
    if factory.floor >= factory.ceiling || factory.ceiling > 100 {
        bail!("原厂节能自动阈值无效");
    }
    Ok(Some(Capability {
        factory,
        factory_defaulted: defaulted,
    }))
}
pub fn commands(policy: &Policy, cap: &Capability, clock: Clock) -> Result<Vec<u32>> {
    policy.validate(cap)?;
    if clock.weekday > 6 || clock.hour > 23 || clock.minute > 59 || clock.second > 59 {
        bail!("本地节能时钟无效");
    }
    let slot = |slot: &Option<Slot>, sub: u32| {
        (sub << 24)
            | slot
                .as_ref()
                .map_or(0, |s| u32::from(s.start) << 6 | u32::from(s.end) << 17)
    };
    if !policy.enabled {
        // Clearing slots separates stopping the schedule from restoring charge protection's shared enable bit.
        return Ok(vec![0x05000000, 0x03000000, 0x04000000]);
    }
    let days = if policy.every_day {
        255
    } else {
        policy.weekdays
    };
    Ok(vec![
        0x05000000,
        0x01000000
            | u32::from(clock.second)
            | u32::from(clock.minute) << 6
            | u32::from(clock.hour) << 12
            | u32::from(clock.weekday) << 17,
        0x02000000 | u32::from(days) | u32::from(policy.limits.floor) << 8,
        slot(&policy.slots[0], 3),
        slot(&policy.slots[1], 4),
        0x06000000 | u32::from(policy.limits.floor) | u32::from(policy.limits.ceiling) << 8,
        0x05000001,
    ])
}
