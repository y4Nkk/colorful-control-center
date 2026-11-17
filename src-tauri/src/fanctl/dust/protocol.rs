use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Schedule {
    pub weekdays: u8,
    pub hour: u8,
    pub on_resume: bool,
}
impl Schedule {
    pub fn validate(&self) -> Result<()> {
        if self.weekdays > 127 || self.hour > 23 {
            bail!("除尘星期或整点无效");
        }
        Ok(())
    }
    pub fn enabled(&self) -> bool {
        self.weekdays != 0 || self.on_resume
    }
    pub fn command(&self) -> u32 {
        u32::from(self.weekdays) | u32::from(self.on_resume) << 7 | u32::from(self.hour) << 8
    }
    pub fn saved(&self) -> [u8; 5] {
        [
            1,
            self.weekdays | u8::from(self.on_resume) << 7,
            self.hour,
            0,
            0,
        ]
    }
}
pub fn decode(saved: &[u8; 5]) -> Result<Option<Schedule>> {
    match saved[0] {
        0 => Ok(None),
        1 => {
            let schedule = Schedule {
                weekdays: saved[1] & 127,
                on_resume: saved[1] & 128 != 0,
                hour: saved[2],
            };
            schedule.validate()?;
            Ok(Some(schedule))
        }
        _ => bail!("未知除尘计划保存标记"),
    }
}
pub fn supported(page: &[u8]) -> Result<bool> {
    if page.len() != 12 {
        bail!("除尘能力页长度无效");
    }
    match &page[..2] {
        [0, 0] => Ok(false),
        [1, 0] => Ok(page[10] & 128 != 0),
        _ => bail!("未知除尘能力页版本"),
    }
}
pub fn clock(weekday: u16, hour: u16, minute: u16, second: u16) -> Result<u32> {
    if weekday > 6 || hour > 23 || minute > 59 || second > 59 {
        bail!("除尘计划的本地时钟无效");
    }
    Ok(u32::from(second)
        | u32::from(minute) << 6
        | u32::from(hour) << 12
        | u32::from(weekday) << 17)
}
