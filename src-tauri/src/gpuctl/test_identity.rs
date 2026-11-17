use crate::hal::nvapi::{
    clock::LockTable,
    pstates::ParsedPstates,
    turbo::{DeviceId, TurboIo},
};
use anyhow::{Result, bail};
use std::sync::Mutex;

pub struct Identity(pub Mutex<DeviceId>);
impl Default for Identity {
    fn default() -> Self {
        Self(Mutex::new(DeviceId {
            pci_device: 1,
            subsystem: 2,
            revision: 3,
            extended: 4,
            bus: 5,
            slot: 6,
        }))
    }
}
impl TurboIo for Identity {
    fn devices(&self) -> Result<Vec<DeviceId>> {
        Ok(vec![*self.0.lock().unwrap()])
    }
    fn primary(&self) -> Result<DeviceId> {
        Ok(*self.0.lock().unwrap())
    }
    fn base_clock(&self, _: &DeviceId) -> Result<u32> {
        bail!("隔离测试未注入 GPU 基础频率")
    }
    fn locks(&self, _: &DeviceId) -> Result<LockTable> {
        bail!("隔离测试未注入 GPU 锁频")
    }
    fn write_locks(&self, _: &DeviceId, _: &LockTable) -> Result<()> {
        bail!("隔离测试禁止 GPU 锁频")
    }
    fn offsets(&self, _: &DeviceId) -> Result<ParsedPstates> {
        bail!("隔离测试未注入 GPU 偏移")
    }
    fn write_offsets(&self, _: &DeviceId, _: Option<i32>, _: Option<i32>) -> Result<()> {
        bail!("隔离测试禁止 GPU 偏移")
    }
}
