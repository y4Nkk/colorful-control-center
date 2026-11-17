use crate::{battery::PowerStatus, hal::Hal};
use anyhow::{Context, Result, bail};
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Trigger {
    Event,
    Startup,
    Resume,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Warning {
    pub id: u64,
    pub temperature_c: i32,
    pub battery_percent: u8,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct Rule {
    version: [u8; 2],
    ui: u8,
    style: Option<u8>,
}
impl Rule {
    fn identity(&self) -> Result<String> {
        super::model::digest(&("battery-cold-warning-1", self))
    }
}
trait Io {
    fn query(&self, request: &[u8; 256]) -> Result<[u8; 256]>;
    fn power(&self) -> Result<PowerStatus>;
}
struct Native<'a>(&'a Hal);
impl Io for Native<'_> {
    fn query(&self, request: &[u8; 256]) -> Result<[u8; 256]> {
        self.0.with_dchu(|io| io.exchange_package(4, request))
    }
    fn power(&self) -> Result<PowerStatus> {
        crate::battery::system_power()
    }
}
fn packet(selector: u8) -> [u8; 256] {
    let mut value = [0; 256];
    value[0] = selector;
    value
}
fn rule(io: &impl Io) -> Result<Rule> {
    let data = io.query(&packet(8)).context("读取原厂低温提醒能力")?;
    let version = [data[0], data[1]];
    if !matches!(version, [0, 0] | [1, 0]) {
        bail!("原厂低温提醒能力版本未知");
    }
    let ui = data[17] >> 4;
    let style = if matches!(ui, 1 | 2) {
        None
    } else {
        Some(io.query(&packet(35)).context("读取原厂界面样式")?[0])
    };
    if !matches!(ui, 1 | 2) && style != Some(3) {
        bail!("当前原厂能力未提供低温提醒");
    }
    Ok(Rule { version, ui, style })
}
fn temperature(io: &impl Io) -> Result<i32> {
    let mut request = packet(1);
    request[1..7].copy_from_slice(&[12, 90, 22, 8, 0, 223]);
    let response = io.query(&request).context("读取原厂电池温度")?;
    let bytes = [response[1], response[2]];
    let raw = u16::from_le_bytes(bytes);
    if matches!(raw, 0 | u16::MAX) || response[1..6] == request[1..6] {
        bail!("原厂电池温度报告无效或仅有请求回声");
    }
    Ok((i32::from(raw) - 2732) / 10)
}
fn inspect(io: &impl Io, expected: &str, trigger: Trigger) -> Result<Option<(i32, u8)>> {
    if rule(io)?.identity()? != expected {
        bail!("低温提醒能力已变化，请重新确认范围");
    }
    if trigger != Trigger::Event {
        match io.power()?.ac_online {
            Some(false) => (),
            Some(true) => return Ok(None),
            None => bail!("供电状态未知，未执行启动或唤醒温度检查"),
        }
    }
    let temperature = temperature(io)?;
    let power = io.power()?;
    if power.present != Some(true) {
        bail!("未确认电池存在，未生成低温提醒");
    }
    let percent = power.percent.context("电池电量未知，未生成低温提醒")?;
    if percent > 100 {
        bail!("电池电量超出百分比范围，未生成低温提醒");
    }
    Ok((temperature < 11 && percent < 81).then_some((temperature, percent)))
}
pub(super) fn identity(hal: &Hal) -> Result<String> {
    let _gate = hal.begin_write()?;
    rule(&Native(hal))?.identity()
}
pub(super) fn read(hal: &Hal, expected: &str, trigger: Trigger) -> Result<Option<(i32, u8)>> {
    let _gate = hal.begin_write()?;
    inspect(&Native(hal), expected, trigger)
}

#[cfg(test)]
#[path = "battery_cold/ffi_tests.rs"]
mod ffi_tests;
#[cfg(test)]
#[path = "battery_cold/tests.rs"]
mod tests;
