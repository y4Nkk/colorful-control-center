use super::*;
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

#[derive(Clone)]
enum Event {
    Primed,
    Value(u32, f64),
    /// 效用与性能百分比同一次采样返回；基频计数器缺失。
    ValueWithPerformance(u32, f64, u32, f64),
    /// 效用、性能百分比与标称基频同一次采样返回；频率 = 基频 × 性能百分比。
    ValueWithBase(u32, f64, u32, f64, u32, f64),
    Error,
}
#[derive(Default)]
struct Data {
    events: VecDeque<Event>,
    calls: usize,
    resets: usize,
}
#[derive(Clone)]
struct Memory(Arc<Mutex<Data>>);
impl Memory {
    fn new(events: impl IntoIterator<Item = Event>) -> Self {
        Self(Arc::new(Mutex::new(Data {
            events: events.into_iter().collect(),
            ..Data::default()
        })))
    }
}
impl Counter for Memory {
    fn collect(&mut self) -> Result<Option<Sample>> {
        let mut data = self.0.lock().unwrap();
        data.calls += 1;
        match data.events.pop_front().expect("测试没有提供CPU计数器样本") {
            Event::Primed => Ok(None),
            Event::Value(status, value) => Ok(Some(Sample {
                utility: (status, value),
                performance: None,
                base_clock: None,
            })),
            Event::ValueWithPerformance(utility_status, utility, performance_status, performance) => {
                Ok(Some(Sample {
                    utility: (utility_status, utility),
                    performance: Some((performance_status, performance)),
                    base_clock: None,
                }))
            }
            Event::ValueWithBase(
                utility_status,
                utility,
                performance_status,
                performance,
                base_status,
                base,
            ) => Ok(Some(Sample {
                utility: (utility_status, utility),
                performance: Some((performance_status, performance)),
                base_clock: Some((base_status, base)),
            })),
            Event::Error => bail!("隔离计数器失败"),
        }
    }
    fn reset(&mut self) {
        self.0.lock().unwrap().resets += 1;
    }
}
#[test]
fn first_sample_is_unknown_then_short_requests_reuse_the_timestamped_reading() {
    let io = Memory::new([Event::Primed, Event::Value(1, 42.7)]);
    let mut sampler = Sampler::with_counter(io.clone());
    let start = Instant::now();
    assert_eq!(sampler.sample(start, 1000).unwrap(), None);
    assert_eq!(
        sampler.sample(start + Duration::from_millis(300), 1300).unwrap(),
        None
    );
    let reading = sampler.sample(start + INTERVAL, 1750).unwrap().unwrap();
    assert_eq!((reading.value_pct, reading.sampled_at_ms), (42.7, 1750));
    assert_eq!(reading.clock_mhz, None);
    assert_eq!(
        sampler.sample(start + Duration::from_millis(900), 1900).unwrap(),
        Some(reading)
    );
    assert_eq!(io.0.lock().unwrap().calls, 2);
}
#[test]
fn failure_clears_old_value_throttles_retries_and_requires_new_baseline() {
    let io = Memory::new([
        Event::Primed,
        Event::Value(0, 35.0),
        Event::Error,
        Event::Primed,
        Event::Value(1, 55.0),
    ]);
    let mut sampler = Sampler::with_counter(io.clone());
    let t = Instant::now();
    assert!(sampler.sample(t, 1000).unwrap().is_none());
    assert!(sampler.sample(t + INTERVAL, 1750).unwrap().is_some());
    assert!(sampler.sample(t + INTERVAL * 2, 2500).is_err());
    assert!(
        sampler
            .sample(t + INTERVAL * 2 + Duration::from_millis(20), 2520)
            .is_err()
    );
    assert_eq!(io.0.lock().unwrap().calls, 3);
    assert!(sampler.sample(t + INTERVAL * 3, 3250).unwrap().is_none());
    assert_eq!(
        sampler.sample(t + INTERVAL * 4, 4000).unwrap().unwrap().value_pct,
        55.0
    );
    assert_eq!(io.0.lock().unwrap().resets, 1);
}
#[test]
fn counter_status_and_numeric_boundaries_never_turn_invalid_data_into_zero() {
    for (status, value) in [
        (2, 0.0),
        (0xc0000bc6, 20.0),
        (0, -0.01),
        (0, f64::NAN),
        (0, f64::INFINITY),
    ] {
        let io = Memory::new([Event::Value(status, value)]);
        let mut sampler = Sampler::with_counter(io.clone());
        assert!(sampler.sample(Instant::now(), 1000).is_err());
        assert_eq!(io.0.lock().unwrap().resets, 1);
    }
    assert_eq!(validate_value(0, 0.0).unwrap(), 0.0);
    assert_eq!(validate_value(1, 127.0).unwrap(), 100.0);
}
#[test]
fn performance_ratio_turns_into_base_clock_frequency() {
    let io = Memory::new([Event::ValueWithBase(0, 45.0, 0, 187.5, 1, 2400.0)]);
    let mut sampler = Sampler::with_counter(io);
    let reading = sampler.sample(Instant::now(), 1000).unwrap().unwrap();
    assert_eq!(reading.value_pct, 45.0);
    assert_eq!(reading.clock_mhz, Some(4500.0));
}
#[test]
fn turbo_performance_exceeds_nominal_clock_without_clamping() {
    let io = Memory::new([Event::ValueWithBase(1, 100.0, 1, 208.3, 1, 2400.0)]);
    let mut sampler = Sampler::with_counter(io);
    let reading = sampler.sample(Instant::now(), 1000).unwrap().unwrap();
    assert_eq!(validate_performance(1, 208.3).unwrap(), 208.3);
    assert_eq!(reading.clock_mhz, Some(2400.0 * 208.3 / 100.0));
}
#[test]
fn invalid_performance_keeps_utility_and_leaves_clock_empty() {
    for (status, value) in [(2, 187.5), (0xc0000bc6, 187.5), (0, -1.0), (0, f64::NAN)] {
        let io = Memory::new([Event::ValueWithBase(0, 45.0, status, value, 1, 2400.0)]);
        let mut sampler = Sampler::with_counter(io);
        let reading = sampler.sample(Instant::now(), 1000).unwrap().unwrap();
        assert_eq!(reading.value_pct, 45.0);
        assert_eq!(reading.clock_mhz, None);
    }
}
#[test]
fn missing_base_clock_leaves_frequency_empty_even_when_performance_is_valid() {
    let io = Memory::new([Event::ValueWithPerformance(0, 45.0, 0, 187.5)]);
    let mut sampler = Sampler::with_counter(io);
    let reading = sampler.sample(Instant::now(), 1000).unwrap().unwrap();
    assert_eq!(reading.clock_mhz, None);
}
#[test]
fn invalid_base_clock_keeps_utility_and_leaves_clock_empty() {
    for (status, value) in [
        (2, 2220.0),
        (0xc0000bc6, 2220.0),
        (0, -1.0),
        (0, 0.0),
        (0, f64::NAN),
        (0, f64::INFINITY),
    ] {
        let io = Memory::new([Event::ValueWithBase(0, 45.0, 1, 187.5, status, value)]);
        let mut sampler = Sampler::with_counter(io);
        let reading = sampler.sample(Instant::now(), 1000).unwrap().unwrap();
        assert_eq!(reading.value_pct, 45.0);
        assert_eq!(reading.clock_mhz, None);
    }
}
#[test]
fn long_gap_and_clock_reversal_discard_previous_window_and_sample_again() {
    let io = Memory::new([
        Event::Primed,
        Event::Value(0, 25.0),
        Event::Primed,
        Event::Value(0, 40.0),
        Event::Primed,
    ]);
    let mut sampler = Sampler::with_counter(io.clone());
    let t = Instant::now();
    sampler.sample(t, 1000).unwrap();
    sampler.sample(t + INTERVAL, 1750).unwrap();
    assert!(
        sampler
            .sample(t + Duration::from_secs(8), 9000)
            .unwrap()
            .is_none()
    );
    assert_eq!(
        sampler
            .sample(t + Duration::from_secs(9), 10000)
            .unwrap()
            .unwrap()
            .value_pct,
        40.0
    );
    assert!(sampler.sample(t + Duration::from_secs(10), 900).unwrap().is_none());
    assert_eq!(io.0.lock().unwrap().resets, 2);
}
#[test]
fn concurrent_requests_share_one_counter_sample() {
    let io = Memory::new([Event::Value(0, 27.0)]);
    let sampler = Arc::new(Mutex::new(Sampler::with_counter(io.clone())));
    let barrier = Arc::new(std::sync::Barrier::new(8));
    let now = Instant::now();
    let threads: Vec<_> = (0..8)
        .map(|_| {
            let sampler = sampler.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                sampler.lock().unwrap().sample(now, 1000).unwrap().unwrap()
            })
        })
        .collect();
    for thread in threads {
        assert_eq!(thread.join().unwrap().value_pct, 27.0);
    }
    assert_eq!(io.0.lock().unwrap().calls, 1);
}
#[test]
fn missing_test_provider_is_explicitly_unavailable() {
    assert!(
        Sampler::default()
            .sample(Instant::now(), 1000)
            .unwrap_err()
            .to_string()
            .contains("未注入")
    );
}

unsafe extern "system" fn buffer(_: i32, _: *mut u8) -> i32 {
    0
}
unsafe extern "system" fn read(_: i32, _: i32, _: i32, _: *mut u8) -> i32 {
    0
}
unsafe extern "system" fn set(_: i32, _: *const u8, _: i32) -> i32 {
    0
}
unsafe extern "system" fn write(_: i32, _: i32, _: i32, _: *const u8) -> i32 {
    0
}
#[test]
fn display_sampling_is_separate_from_fan_and_safety_snapshots() {
    let io = Memory::new([Event::Value(0, 42.0)]);
    let hal = super::super::Hal::test_instance(
        super::super::oem_dchu::Dchu::test_io(buffer, read, set, write),
        None,
    );
    *hal.cpu_utility.lock().unwrap() = Sampler::with_counter(io.clone());
    assert!(hal.snapshot().cpu_utility.is_none());
    assert_eq!(io.0.lock().unwrap().calls, 0);
    assert_eq!(hal.display_snapshot().cpu_utility.unwrap().value_pct, 42.0);
    assert_eq!(io.0.lock().unwrap().calls, 1);
    assert!(hal.snapshot().cpu_utility.is_none());
    assert_eq!(io.0.lock().unwrap().calls, 1);
}
