use super::{monitor::TimedReading, oem_dchu::Dchu};
use anyhow::{Context, Result, ensure};
use serde::Serialize;
use std::time::{Duration, Instant};

const REPORTED_CLOCK_TTL: Duration = Duration::from_secs(5);
const REFERENCE_CLOCK_TTL: Duration = Duration::from_secs(30);

#[derive(Clone, Serialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct OemMemoryReading {
    pub reported_clock_mhz: Option<u32>,
    pub reference_clock_mhz: Option<u32>,
}

#[derive(Default)]
pub(super) struct DisplayCache {
    reported: TimedReading<Option<u32>>,
    reference: TimedReading<Option<u32>>,
}

impl DisplayCache {
    pub(super) fn read(
        &mut self,
        io: &Dchu,
        now: Instant,
        warnings: &mut Vec<String>,
    ) -> Option<OemMemoryReading> {
        let reported = self
            .reported
            .read(now, REPORTED_CLOCK_TTL, || reported_clock(io));
        let reference = self
            .reference
            .read(now, REFERENCE_CLOCK_TTL, || reference_clock(io));
        reading(reported, reference, warnings)
    }
}

pub(super) fn read(io: &Dchu, warnings: &mut Vec<String>) -> Option<OemMemoryReading> {
    reading(reported_clock(io), reference_clock(io), warnings)
}

fn reported_clock(io: &Dchu) -> Result<Option<u32>> {
    let bytes = io
        .read_settings(6, 23, 4)
        .context("内存原厂报告频率读取失败")?;
    let value = i32::from_le_bytes(bytes.try_into().unwrap());
    ensure!(value >= 0, "内存原厂报告频率无效：{value}");
    Ok((value > 0).then_some(value as u32))
}

fn reference_clock(io: &Dchu) -> Result<Option<u32>> {
    let bytes = io
        .read_settings(6, 230, 2)
        .context("内存原厂参考频率读取失败")?;
    let value = u16::from_le_bytes(bytes.try_into().unwrap());
    Ok((value > 0).then_some(u32::from(value)))
}

fn reading(
    reported: Result<Option<u32>>,
    reference: Result<Option<u32>>,
    warnings: &mut Vec<String>,
) -> Option<OemMemoryReading> {
    let mut keep = |result| match result {
        Ok(value) => value,
        Err(error) => {
            warnings.push(format!("{error:#}"));
            None
        }
    };
    let reported_clock_mhz = keep(reported);
    let reference_clock_mhz = keep(reference);
    (reported_clock_mhz.is_some() || reference_clock_mhz.is_some()).then_some(OemMemoryReading {
        reported_clock_mhz,
        reference_clock_mhz,
    })
}

#[cfg(test)]
#[path = "memory_tests.rs"]
mod tests;
