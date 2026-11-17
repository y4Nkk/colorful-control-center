use super::{Record, parse};
use anyhow::{Context, Result, bail};
use windows_sys::{
    Win32::{
        Foundation::{ERROR_HANDLE_EOF, ERROR_INSUFFICIENT_BUFFER, HANDLE},
        System::{EventLog::*, Registry::*},
    },
    core::w,
};
const EVENTLOG_FORWARDS_READ: u32 = 4;

pub(crate) fn ensure_log(name: &str) -> Result<()> {
    let key_name = match name {
        "System" => w!("SYSTEM\\CurrentControlSet\\Services\\EventLog\\System"),
        "OutLog" => w!("SYSTEM\\CurrentControlSet\\Services\\EventLog\\OutLog"),
        "PowerBiosServerLog" => {
            w!("SYSTEM\\CurrentControlSet\\Services\\EventLog\\PowerBiosServerLog")
        }
        _ => bail!("未声明的事件日志"),
    };
    let mut key = std::ptr::null_mut();
    let status = unsafe { RegOpenKeyExW(HKEY_LOCAL_MACHINE, key_name, 0, KEY_READ, &mut key) };
    if status != 0 {
        bail!(
            "{name}日志不可用：{}",
            std::io::Error::from_raw_os_error(status as i32)
        );
    }
    unsafe {
        RegCloseKey(key);
    }
    Ok(())
}
pub(crate) struct Log {
    handle: HANDLE,
    cursor: u32,
}
impl Drop for Log {
    fn drop(&mut self) {
        unsafe {
            CloseEventLog(self.handle);
        }
    }
}
impl Log {
    pub fn open(name: &str) -> Result<Self> {
        // OpenEventLog silently falls back to Application if the exact registry log is absent.
        ensure_log(name)?;
        let name: Vec<_> = name.encode_utf16().chain([0]).collect();
        let handle = unsafe { OpenEventLogW(std::ptr::null(), name.as_ptr()) };
        if handle.is_null() {
            return Err(std::io::Error::last_os_error().into());
        }
        let mut log = Self { handle, cursor: 1 };
        log.cursor = log.extent()?.1;
        Ok(log)
    }
    fn extent(&self) -> Result<(u32, u32)> {
        let mut count = 0;
        let mut oldest = 0;
        if unsafe { GetNumberOfEventLogRecords(self.handle, &mut count) } == 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        if count == 0 {
            return Ok((1, 1));
        }
        if unsafe { GetOldestEventLogRecord(self.handle, &mut oldest) } == 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        Ok((
            oldest,
            oldest.checked_add(count).context("事件日志序号溢出")?,
        ))
    }
    pub fn poll(&mut self) -> Result<Vec<Record>> {
        let (oldest, end) = self.extent()?;
        if self.cursor < oldest || self.cursor > end {
            bail!("事件日志已清空或覆盖，订阅停止");
        }
        if self.cursor == end {
            return Ok(Vec::new());
        }
        let mut buffer = vec![0; 65_536];
        let bytes = self.read(&mut buffer)?;
        let (next, events) = parse(&buffer[..bytes], self.cursor)?;
        self.cursor = next;
        Ok(events)
    }
    pub fn discard_pending(&mut self) -> Result<()> {
        let (oldest, end) = self.extent()?;
        if self.cursor < oldest || self.cursor > end {
            bail!("事件日志已清空或覆盖，订阅停止");
        }
        self.cursor = end;
        Ok(())
    }
    fn read(&self, buffer: &mut Vec<u8>) -> Result<usize> {
        loop {
            let mut bytes = 0;
            let mut needed = 0;
            let ok = unsafe {
                ReadEventLogW(
                    self.handle,
                    EVENTLOG_SEEK_READ | EVENTLOG_FORWARDS_READ,
                    self.cursor,
                    buffer.as_mut_ptr().cast(),
                    buffer.len() as u32,
                    &mut bytes,
                    &mut needed,
                )
            };
            if ok != 0 {
                return Ok(bytes as usize);
            }
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() == Some(ERROR_HANDLE_EOF as i32) {
                return Ok(0);
            }
            if error.raw_os_error() == Some(ERROR_INSUFFICIENT_BUFFER as i32)
                && needed as usize > buffer.len()
                && needed <= 1_048_576
            {
                buffer.resize(needed as usize, 0);
                continue;
            }
            return Err(error.into());
        }
    }
}
