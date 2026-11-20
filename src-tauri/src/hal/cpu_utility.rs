use anyhow::{Result, anyhow, bail};
use serde::Serialize;
use std::time::{Duration, Instant};

#[cfg(not(test))]
mod native;
#[cfg(test)]
mod tests;

const INTERVAL: Duration = Duration::from_millis(750);
const RESET_AFTER: Duration = Duration::from_secs(5);
const MAX_AGE_MS: u64 = 2500;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Reading {
    pub value_pct: f64,
    /// PDH 标称基频 × 处理器性能百分比；基频计数缺失或无效时为空。
    pub clock_mhz: Option<f64>,
    pub sampled_at_ms: u64,
}
/// 同一次 PDH 采样的效用、性能百分比与标称基频；性能或基频计数缺失（系统未提供）时为 None。
pub(super) struct Sample {
    utility: (u32, f64),
    performance: Option<(u32, f64)>,
    base_clock: Option<(u32, f64)>,
}
trait Counter: Send {
    fn collect(&mut self) -> Result<Option<Sample>>;
    fn reset(&mut self);
}
pub(super) struct Sampler {
    source: Option<Box<dyn Counter>>,
    last_attempt: Option<Instant>,
    last_reading: Option<Reading>,
    last_error: Option<String>,
}
impl Default for Sampler {
    fn default() -> Self {
        Self {
            #[cfg(not(test))]
            source: Some(Box::new(native::Counter::default())),
            #[cfg(test)]
            source: None,
            last_attempt: None,
            last_reading: None,
            last_error: None,
        }
    }
}
impl Sampler {
    #[cfg(test)]
    fn with_counter(source: impl Counter + 'static) -> Self {
        Self {
            source: Some(Box::new(source)),
            ..Self::default()
        }
    }
    pub(super) fn sample(&mut self, now: Instant, sampled_at_ms: u64) -> Result<Option<Reading>> {
        if sampled_at_ms == 0 {
            return self.failure("系统采样时间不可用".into());
        }
        if let Some(last) = self.last_attempt {
            let elapsed = now.checked_duration_since(last);
            let clock_reversed = self
                .last_reading
                .as_ref()
                .is_some_and(|value| sampled_at_ms < value.sampled_at_ms);
            if elapsed.is_none()
                || elapsed.is_some_and(|value| value >= RESET_AFTER)
                || clock_reversed
            {
                self.reset();
            } else if elapsed.unwrap() < INTERVAL {
                if let Some(error) = &self.last_error {
                    bail!("{error}");
                }
                return Ok(self
                    .last_reading
                    .clone()
                    .filter(|value| sampled_at_ms - value.sampled_at_ms <= MAX_AGE_MS));
            }
        }
        self.last_attempt = Some(now);
        let result = self
            .source
            .as_mut()
            .ok_or_else(|| anyhow!("隔离测试未注入CPU效用计数器"))
            .and_then(|counter| counter.collect())
            .and_then(|value| {
                value
                    .map(|sample| reading_from(sample, sampled_at_ms))
                    .transpose()
            });
        match result {
            Ok(value) => {
                self.last_error = None;
                self.last_reading = value;
                Ok(self.last_reading.clone())
            }
            Err(error) => self.failure(format!("{error:#}")),
        }
    }
    fn reset(&mut self) {
        if let Some(source) = &mut self.source {
            source.reset();
        }
        self.last_attempt = None;
        self.last_reading = None;
        self.last_error = None;
    }
    fn failure(&mut self, error: String) -> Result<Option<Reading>> {
        if let Some(source) = &mut self.source {
            source.reset();
        }
        self.last_reading = None;
        self.last_error = Some(error.clone());
        Err(anyhow!(error))
    }
}
fn validate_value(status: u32, value: f64) -> Result<f64> {
    if ![0, 1].contains(&status) {
        bail!("CPU效用计数器数据状态无效：{status:#010x}");
    }
    if !value.is_finite() || value < 0.0 {
        bail!("CPU效用计数器数值无效：{value}");
    }
    Ok(value.min(100.0))
}
fn validate_performance(status: u32, value: f64) -> Result<f64> {
    if ![0, 1].contains(&status) {
        bail!("CPU性能计数器数据状态无效：{status:#010x}");
    }
    if !value.is_finite() || value < 0.0 {
        bail!("CPU性能计数器数值无效：{value}");
    }
    // 性能百分比在睿频时超过 100，不设上限。
    Ok(value)
}
fn validate_base_clock(status: u32, value: f64) -> Result<f64> {
    if ![0, 1].contains(&status) {
        bail!("CPU基频计数器数据状态无效：{status:#010x}");
    }
    if !value.is_finite() || value <= 0.0 {
        bail!("CPU基频计数器数值无效：{value}");
    }
    Ok(value)
}
/// 三项读数来自同一次采样；性能或基频计数无效只让 clock 为空，不拖垮效用读数。
fn reading_from(sample: Sample, sampled_at_ms: u64) -> Result<Reading> {
    let value_pct = validate_value(sample.utility.0, sample.utility.1)?;
    let performance = sample
        .performance
        .map(|(status, value)| validate_performance(status, value))
        .transpose()
        .ok()
        .flatten();
    let base_clock = sample
        .base_clock
        .map(|(status, value)| validate_base_clock(status, value))
        .transpose()
        .ok()
        .flatten();
    let clock_mhz = base_clock
        .zip(performance)
        .map(|(base, pct)| base * pct / 100.0)
        .filter(|mhz| mhz.is_finite() && *mhz > 0.0);
    Ok(Reading {
        value_pct,
        clock_mhz,
        sampled_at_ms,
    })
}
