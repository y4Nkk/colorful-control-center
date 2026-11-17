//! MSR 会话边界；测试仅接受显式内存传输，不打开真实设备。
use anyhow::{Result, anyhow};
use ccc_core::msr::MsrAccess;

pub struct MsrIo {
    #[cfg(not(test))]
    inner: super::pawnio::PawnIo,
    #[cfg(test)]
    inner: std::sync::Arc<dyn MsrAccess + Send + Sync>,
}

impl MsrIo {
    pub(super) fn open(hal: &super::Hal) -> Result<Self> {
        #[cfg(test)]
        {
            return Ok(Self {
                inner: hal
                    .test_msr
                    .clone()
                    .ok_or_else(|| anyhow!("隔离测试未配置 MSR 传输"))?,
            });
        }
        #[cfg(not(test))]
        {
            let _ = hal;
            Ok(Self {
                inner: super::pawnio::PawnIo::open_with_intel_msr()?,
            })
        }
    }
    pub fn read_msr(&self, address: u32) -> Result<u64> {
        self.read(address).map_err(|e| anyhow!(e))
    }
}

impl MsrAccess for MsrIo {
    fn read(&self, address: u32) -> std::result::Result<u64, String> {
        self.inner.read(address)
    }
    fn write(&self, address: u32, value: u64) -> std::result::Result<(), String> {
        self.inner.write(address, value)
    }
    fn pause(&self) {
        self.inner.pause();
    }
}
