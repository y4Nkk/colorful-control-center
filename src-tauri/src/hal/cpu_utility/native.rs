use super::Counter as CounterAccess;
use anyhow::{Result, bail};
use windows_sys::Win32::System::Performance::*;

#[derive(Default)]
pub(super) struct Counter {
    query: Option<Query>,
}
struct Query {
    handle: PDH_HQUERY,
    counter: PDH_HCOUNTER,
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
            counter: std::ptr::null_mut(),
            primed: false,
        };
        let path: Vec<u16> = r"\Processor Information(_Total)\% Processor Utility"
            .encode_utf16()
            .chain([0])
            .collect();
        checked(
            unsafe { PdhAddEnglishCounterW(query.handle, path.as_ptr(), 0, &mut query.counter) },
            "添加CPU效用计数器",
        )?;
        if query.counter.is_null() {
            bail!("CPU效用计数器返回空句柄");
        }
        Ok(query)
    }
    fn collect(&mut self) -> Result<Option<(u32, f64)>> {
        checked(unsafe { PdhCollectQueryData(self.handle) }, "采集CPU效用")?;
        if !self.primed {
            self.primed = true;
            return Ok(None);
        }
        let mut value = PDH_FMT_COUNTERVALUE {
            CStatus: u32::MAX,
            Anonymous: PDH_FMT_COUNTERVALUE_0 {
                doubleValue: f64::NAN,
            },
        };
        checked(
            unsafe {
                PdhGetFormattedCounterValue(
                    self.counter,
                    PDH_FMT_DOUBLE,
                    std::ptr::null_mut(),
                    &mut value,
                )
            },
            "计算CPU效用",
        )?;
        Ok(Some((value.CStatus, unsafe {
            value.Anonymous.doubleValue
        })))
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
    fn collect(&mut self) -> Result<Option<(u32, f64)>> {
        if self.query.is_none() {
            self.query = Some(Query::open()?);
        }
        self.query.as_mut().unwrap().collect()
    }
    fn reset(&mut self) {
        self.query = None;
    }
}
