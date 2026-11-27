use anyhow::{Result, anyhow, bail};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

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
    /// 同一窗口内按忙碌时间加权的活跃逻辑处理器频率估算。
    pub clock_mhz: Option<f64>,
    pub max_active_clock_mhz: Option<f64>,
    pub sampled_at_ms: u64,
}

type CounterValue = (u32, f64);
type ProcessorValues = BTreeMap<(u16, u16), CounterValue>;

struct ClockSample {
    logical_processors: usize,
    busy: ProcessorValues,
    performance: ProcessorValues,
    base_clock: ProcessorValues,
}

pub(super) struct Sample {
    utility: CounterValue,
    clocks: Option<ClockSample>,
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
    pub(super) fn reset(&mut self) {
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
fn valid_counter((status, value): CounterValue) -> Option<f64> {
    ([0, 1].contains(&status) && value.is_finite() && value >= 0.0).then_some(value)
}

impl ClockSample {
    fn estimate(&self) -> Option<(f64, f64)> {
        if self.busy.len() != self.logical_processors {
            return None;
        }
        let mut weighted_clock = 0.0;
        let mut total_busy = 0.0;
        let mut highest: f64 = 0.0;
        for (id, value) in &self.busy {
            let busy = valid_counter(*value).filter(|value| *value <= 100.0)?;
            if busy == 0.0 {
                continue;
            }
            let base = valid_counter(*self.base_clock.get(id)?)?;
            let performance = valid_counter(*self.performance.get(id)?)?;
            let clock = base * (performance / 100.0);
            if !clock.is_finite() || clock <= 0.0 {
                return None;
            }
            weighted_clock += clock * busy;
            total_busy += busy;
            highest = highest.max(clock);
        }
        let average = weighted_clock / total_busy;
        (total_busy > 0.0 && average.is_finite()).then_some((average, highest))
    }
}

fn reading_from(sample: Sample, sampled_at_ms: u64) -> Result<Reading> {
    let value_pct = validate_value(sample.utility.0, sample.utility.1)?;
    let clocks = sample.clocks.and_then(|sample| sample.estimate());
    Ok(Reading {
        value_pct,
        clock_mhz: clocks.map(|(average, _)| average),
        max_active_clock_mhz: clocks.map(|(_, highest)| highest),
        sampled_at_ms,
    })
}
