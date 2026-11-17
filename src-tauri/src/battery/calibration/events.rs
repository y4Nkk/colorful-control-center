//! 校准只消费原厂服务的三个通知，传输及记录校验统一归HAL。
use super::Event;
use crate::hal::event_log;
use anyhow::{Result, bail};

fn select(records: Vec<event_log::Record>) -> Result<Vec<Event>> {
    let mut events = Vec::new();
    for record in records {
        if record.source != "PowerBIOSServer_Out" || record.strings.len() != 1 {
            continue;
        }
        let event = match record.strings[0].as_str() {
            "Arg001^85" => Event::FullStatus,
            "Arg001^90" => Event::AcUnplugged,
            "Arg001^91" => Event::FullyDischarged,
            _ => continue,
        };
        events.push(event);
        if events.len() > 64 {
            bail!("校准事件积压超过限制");
        }
    }
    Ok(events)
}
#[cfg(test)]
pub(super) fn parse(raw: &[u8], cursor: u32) -> Result<(u32, Vec<Event>)> {
    let (next, records) = event_log::parse(raw, cursor)?;
    Ok((next, select(records)?))
}
pub(super) fn ensure_logs() -> Result<()> {
    event_log::ensure_log("OutLog")?;
    event_log::ensure_log("PowerBiosServerLog")
}
pub(super) struct Log(event_log::Log);
impl Log {
    pub fn open() -> Result<Self> {
        ensure_logs()?;
        Ok(Self(event_log::Log::open("OutLog")?))
    }
    pub fn poll(&mut self) -> Result<Vec<Event>> {
        select(self.0.poll()?)
    }
}
