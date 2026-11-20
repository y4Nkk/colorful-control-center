use super::{Counter as CounterAccess, Sample};
use anyhow::{Result, bail};
use windows_sys::Win32::System::Performance::*;

#[derive(Default)]
pub(super) struct Counter {
    query: Option<Query>,
}
struct Query {
    handle: PDH_HQUERY,
    utility: PDH_HCOUNTER,
    performance: Option<PDH_HCOUNTER>,
    frequency: Option<PDH_HCOUNTER>,
    primed: bool,
}
// 查询句柄由采样器的互斥锁独占，PDH负责内部计数器生命周期。
unsafe impl Send for Query {}
fn checked(status: u32, operation: &str) -> Result<()> {
    if status != 0 {
        bail!("{operation}失败：PDH {status:#010x}");
    }
    Ok(())
}
fn add_counter(
    handle: PDH_HQUERY,
    path: &str,
    slot: &mut PDH_HCOUNTER,
    operation: &str,
) -> Result<()> {
    let wide: Vec<u16> = path.encode_utf16().chain([0]).collect();
    checked(
        unsafe { PdhAddEnglishCounterW(handle, wide.as_ptr(), 0, slot) },
        operation,
    )
}
impl Query {
    fn open() -> Result<Self> {
        let mut handle = std::ptr::null_mut();
        checked(
            unsafe { PdhOpenQueryW(std::ptr::null(), 0, &mut handle) },
            "打开CPU效用计数器",
        )?;
        if handle.is_null() {
            bail!("CPU效用计数器返回空查询句柄");
        }
        let mut query = Self {
            handle,
            utility: std::ptr::null_mut(),
            performance: None,
            frequency: None,
            primed: false,
        };
        add_counter(
            query.handle,
            r"\Processor Information(_Total)\% Processor Utility",
            &mut query.utility,
            "添加CPU效用计数器",
        )?;
        if query.utility.is_null() {
            bail!("CPU效用计数器返回空句柄");
        }
        // 性能计数缺席只损失频率读数，不影响效用采样。
        let mut performance = std::ptr::null_mut();
        match add_counter(
            query.handle,
            r"\Processor Information(_Total)\% Processor Performance",
            &mut performance,
            "添加CPU性能计数器",
        ) {
            Ok(()) if !performance.is_null() => query.performance = Some(performance),
            Ok(_) => log::warn!("[cpu-utility] 性能计数器返回空句柄，频率读数留空"),
            Err(error) => log::warn!("[cpu-utility] 性能计数器不可用：{error:#}；频率读数留空"),
        }
        // 标称基频与性能百分比同组配套；缺席时频率读数留空，效用不受影响。
        let mut frequency = std::ptr::null_mut();
        match add_counter(
            query.handle,
            r"\Processor Information(_Total)\Processor Frequency",
            &mut frequency,
            "添加CPU基频计数器",
        ) {
            Ok(()) if !frequency.is_null() => query.frequency = Some(frequency),
            Ok(_) => log::warn!("[cpu-utility] 基频计数器返回空句柄，频率读数留空"),
            Err(error) => log::warn!("[cpu-utility] 基频计数器不可用：{error:#}；频率读数留空"),
        }
        Ok(query)
    }
    fn formatted(&self, counter: PDH_HCOUNTER, operation: &str) -> Result<(u32, f64)> {
        let mut value = PDH_FMT_COUNTERVALUE {
            CStatus: u32::MAX,
            Anonymous: PDH_FMT_COUNTERVALUE_0 {
                doubleValue: f64::NAN,
            },
        };
        checked(
            unsafe {
                PdhGetFormattedCounterValue(counter, PDH_FMT_DOUBLE, std::ptr::null_mut(), &mut value)
            },
            operation,
        )?;
        Ok((value.CStatus, unsafe { value.Anonymous.doubleValue }))
    }
    fn collect(&mut self) -> Result<Option<Sample>> {
        checked(unsafe { PdhCollectQueryData(self.handle) }, "采集CPU效用")?;
        if !self.primed {
            self.primed = true;
            return Ok(None);
        }
        let utility = self.formatted(self.utility, "计算CPU效用")?;
        let performance = self
            .performance
            .map(|counter| self.formatted(counter, "计算CPU性能"))
            .transpose()
            .ok()
            .flatten();
        let base_clock = self
            .frequency
            .map(|counter| self.formatted(counter, "计算CPU基频"))
            .transpose()
            .ok()
            .flatten();
        Ok(Some(Sample {
            utility,
            performance,
            base_clock,
        }))
    }
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
