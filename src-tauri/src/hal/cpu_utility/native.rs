use super::{
    ClockSample, Counter as CounterAccess, CounterValue, ProcessorValues, Sample, valid_counter,
};
use anyhow::{Result, bail, ensure};
use std::{
    mem::{align_of, size_of},
    slice,
};
use windows_sys::Win32::System::{
    Performance::*,
    Threading::{ALL_PROCESSOR_GROUPS, GetActiveProcessorCount},
};

// pdh.h 中的格式修饰符，windows-sys 未导出。
const PDH_FMT_NOCAP100: PDH_FMT = 0x0000_8000;

#[derive(Default)]
pub(super) struct Counter {
    query: Option<Query>,
}

struct Query {
    handle: PDH_HQUERY,
    utility: PDH_HCOUNTER,
    clocks: Option<ClockCounters>,
    previous_bases: Option<ProcessorValues>,
    primed: bool,
}

struct ClockCounters {
    performance: PDH_HCOUNTER,
    frequency: PDH_HCOUNTER,
    busy: PDH_HCOUNTER,
}

// 查询句柄由采样器的互斥锁独占，PDH负责内部计数器生命周期。
unsafe impl Send for Query {}

fn checked(status: u32, operation: &str) -> Result<()> {
    if status != 0 {
        bail!("{operation}失败：PDH {status:#010x}");
    }
    Ok(())
}

fn add_counter(handle: PDH_HQUERY, path: &str) -> Result<PDH_HCOUNTER> {
    let wide: Vec<u16> = path.encode_utf16().chain([0]).collect();
    let mut counter = std::ptr::null_mut();
    checked(
        unsafe { PdhAddEnglishCounterW(handle, wide.as_ptr(), 0, &mut counter) },
        path,
    )?;
    ensure!(!counter.is_null(), "{path}返回空计数器");
    Ok(counter)
}

impl Query {
    fn open() -> Result<Self> {
        let mut handle = std::ptr::null_mut();
        checked(
            unsafe { PdhOpenQueryW(std::ptr::null(), 0, &mut handle) },
            "打开CPU效用计数器",
        )?;
        ensure!(!handle.is_null(), "CPU效用计数器返回空查询句柄");
        let mut query = Self {
            handle,
            utility: std::ptr::null_mut(),
            clocks: None,
            previous_bases: None,
            primed: false,
        };
        query.utility = add_counter(
            handle,
            r"\Processor Information(_Total)\% Processor Utility",
        )?;
        query.clocks = ClockCounters::open(handle)
            .map_err(|error| log::debug!("[cpu-utility] 逐处理器频率计数器不可用：{error:#}"))
            .ok();
        Ok(query)
    }

    fn collect(&mut self) -> Result<Option<Sample>> {
        checked(unsafe { PdhCollectQueryData(self.handle) }, "采集CPU效用")?;
        let clocks = self.clocks.as_ref().and_then(|counters| {
            counters
                .sample(&mut self.previous_bases, self.primed)
                .map_err(|error| log::debug!("[cpu-utility] 逐处理器读数缺失：{error:#}"))
                .ok()
                .flatten()
        });
        if !self.primed {
            self.primed = true;
            return Ok(None);
        }
        let mut utility = PDH_FMT_COUNTERVALUE::default();
        checked(
            unsafe {
                PdhGetFormattedCounterValue(
                    self.utility,
                    PDH_FMT_DOUBLE,
                    std::ptr::null_mut(),
                    &mut utility,
                )
            },
            "计算CPU效用",
        )?;
        Ok(Some(Sample {
            utility: counter_value(utility),
            clocks,
        }))
    }
}

impl ClockCounters {
    fn open(handle: PDH_HQUERY) -> Result<Self> {
        let mut added = Vec::with_capacity(3);
        for path in [
            r"\Processor Information(*)\% Processor Performance",
            r"\Processor Information(*)\Processor Frequency",
            r"\Processor Information(*)\% Processor Time",
        ] {
            match add_counter(handle, path) {
                Ok(counter) => added.push(counter),
                Err(error) => {
                    for counter in added {
                        unsafe { PdhRemoveCounter(counter) };
                    }
                    return Err(error);
                }
            }
        }
        Ok(Self {
            performance: added[0],
            frequency: added[1],
            busy: added[2],
        })
    }

    fn sample(
        &self,
        previous_bases: &mut Option<ProcessorValues>,
        ready: bool,
    ) -> Result<Option<ClockSample>> {
        let current = formatted_processors(self.frequency, PDH_FMT_DOUBLE);
        let previous = previous_bases.take();
        let current = current?;
        // 只有窗口两端基频相符的实例可用于估算。
        let stable = |previous: ProcessorValues| {
            current
                .iter()
                .filter_map(|(id, value)| {
                    let base = valid_counter(*value)?;
                    (previous.get(id).copied().and_then(valid_counter) == Some(base))
                        .then_some((*id, *value))
                })
                .collect()
        };
        let bases = previous.filter(|_| ready).map(stable);
        *previous_bases = Some(current);
        let Some(base_clock) = bases else {
            return Ok(None);
        };
        Ok(Some(ClockSample {
            logical_processors: unsafe { GetActiveProcessorCount(ALL_PROCESSOR_GROUPS) } as usize,
            busy: formatted_processors(self.busy, PDH_FMT_DOUBLE)?,
            performance: formatted_processors(self.performance, PDH_FMT_DOUBLE | PDH_FMT_NOCAP100)?,
            base_clock,
        }))
    }
}

fn counter_value(value: PDH_FMT_COUNTERVALUE) -> CounterValue {
    let status = value.CStatus;
    (status, unsafe { value.Anonymous.doubleValue })
}

fn formatted_processors(counter: PDH_HCOUNTER, format: u32) -> Result<ProcessorValues> {
    let mut size = 0;
    let mut count = 0;
    let status = unsafe {
        PdhGetFormattedCounterArrayW(counter, format, &mut size, &mut count, std::ptr::null_mut())
    };
    ensure!(
        status == PDH_MORE_DATA && (1..=1_048_576).contains(&size),
        "PDH 实例缓冲区不可用：{status:#010x}，{size} 字节"
    );
    const _: () = assert!(align_of::<u64>() >= align_of::<PDH_FMT_COUNTERVALUE_ITEM_W>());
    let mut buffer = vec![0u64; (size as usize).div_ceil(size_of::<u64>())];
    let items = buffer.as_mut_ptr().cast::<PDH_FMT_COUNTERVALUE_ITEM_W>();
    checked(
        unsafe { PdhGetFormattedCounterArrayW(counter, format, &mut size, &mut count, items) },
        "读取PDH逐处理器实例",
    )?;
    ensure!(
        (size as usize) <= buffer.len() * size_of::<u64>()
            && (count as usize) <= size as usize / size_of::<PDH_FMT_COUNTERVALUE_ITEM_W>(),
        "PDH 实例超出缓冲区"
    );
    let mut values = ProcessorValues::new();
    for item in unsafe { slice::from_raw_parts(items, count as usize) } {
        let name = processor_name(item.szName, buffer.as_ptr().addr(), size as usize)?;
        let Some((group, processor)) = name.split_once(',') else {
            continue;
        };
        let (Ok(group), Ok(processor)) = (group.parse::<u16>(), processor.parse::<u16>()) else {
            continue;
        };
        ensure!(
            values
                .insert((group, processor), counter_value(item.FmtValue))
                .is_none(),
            "PDH 逐处理器实例重复：{name}"
        );
    }
    Ok(values)
}

fn processor_name(pointer: *const u16, start: usize, size: usize) -> Result<String> {
    let Some(offset) = pointer.addr().checked_sub(start) else {
        bail!("PDH 实例名超出缓冲区")
    };
    ensure!(
        offset < size && offset % size_of::<u16>() == 0,
        "PDH 实例名地址无效"
    );
    let wide = unsafe { slice::from_raw_parts(pointer, (size - offset) / size_of::<u16>()) };
    let Some(len) = wide.iter().position(|value| *value == 0) else {
        bail!("PDH 实例名不完整")
    };
    Ok(String::from_utf16(&wide[..len])?)
}

impl Drop for Query {
    fn drop(&mut self) {
        let status = unsafe { PdhCloseQuery(self.handle) };
        if status != 0 {
            log::warn!("[cpu-utility] 关闭计数器失败：{status:#010x}");
        }
    }
}

impl CounterAccess for Counter {
    fn collect(&mut self) -> Result<Option<Sample>> {
        if self.query.is_none() {
            self.query = Some(Query::open()?);
        }
        self.query.as_mut().unwrap().collect()
    }

    fn reset(&mut self) {
        self.query = None;
    }
}
