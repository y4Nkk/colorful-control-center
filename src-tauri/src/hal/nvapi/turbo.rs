use super::{clock::LockTable, pstates::ParsedPstates, transaction::PstatesIo};
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeviceId {
    pub pci_device: u32,
    pub subsystem: u32,
    pub revision: u32,
    pub extended: u32,
    pub bus: u32,
    pub slot: u32,
}

pub trait TurboIo: Send + Sync {
    fn devices(&self) -> Result<Vec<DeviceId>>;
    fn primary(&self) -> Result<DeviceId>;
    fn base_clock(&self, id: &DeviceId) -> Result<u32>;
    fn locks(&self, id: &DeviceId) -> Result<LockTable>;
    fn write_locks(&self, id: &DeviceId, table: &LockTable) -> Result<()>;
    fn offsets(&self, id: &DeviceId) -> Result<ParsedPstates>;
    fn write_offsets(&self, id: &DeviceId, core: Option<i32>, memory: Option<i32>) -> Result<()>;
}

pub struct Offsets<'a> {
    pub io: &'a dyn TurboIo,
    pub id: &'a DeviceId,
}
impl PstatesIo for Offsets<'_> {
    fn read(&self) -> Result<ParsedPstates> {
        self.io.offsets(self.id)
    }
    fn write(&self, core: Option<i32>, memory: Option<i32>) -> Result<()> {
        self.io.write_offsets(self.id, core, memory)
    }
}

mod native;
