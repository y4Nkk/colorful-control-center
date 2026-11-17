use super::{DeviceId, TurboIo};
use crate::hal::nvapi::{NvApi, NvApiSession, clock, ffi, pstates};
use anyhow::{Context, Result, anyhow, bail};
use std::ffi::c_void;

type DeviceValue = unsafe extern "system" fn(*mut c_void, *mut u32) -> i32;
type PciInfo =
    unsafe extern "system" fn(*mut c_void, *mut u32, *mut u32, *mut u32, *mut u32) -> i32;
type LockIo = unsafe extern "system" fn(*mut c_void, *mut clock::LockTable) -> i32;
type FrequenciesIo = unsafe extern "system" fn(*mut c_void, *mut clock::Frequencies) -> i32;

macro_rules! resolve {
    ($api:expr, $id:expr, $ty:ty) => {{
        let pointer = unsafe { ($api.query)($id) };
        if pointer.is_null() {
            bail!("NVIDIA 驱动未提供 GPU Turbo 接口 0x{:08X}", $id);
        }
        unsafe { std::mem::transmute::<*mut c_void, $ty>(pointer) }
    }};
}

fn checked(api: &NvApi, status: i32, operation: &str) -> Result<()> {
    if status != ffi::STATUS_OK {
        bail!("{operation}失败：{}", api.status_text(status));
    }
    Ok(())
}

fn identify(api: &NvApi, handle: *mut c_void) -> Result<DeviceId> {
    let pci = resolve!(api, clock::GET_PCI_IDENTIFIERS, PciInfo);
    let bus = resolve!(api, clock::GET_BUS_ID, DeviceValue);
    let slot = resolve!(api, clock::GET_BUS_SLOT_ID, DeviceValue);
    let (mut pci_device, mut subsystem, mut revision, mut extended, mut bus_id, mut slot_id) =
        (u32::MAX, u32::MAX, u32::MAX, u32::MAX, u32::MAX, u32::MAX);
    let status = unsafe {
        pci(
            handle,
            &mut pci_device,
            &mut subsystem,
            &mut revision,
            &mut extended,
        )
    };
    checked(api, status, "读取 GPU PCI 身份")?;
    checked(api, unsafe { bus(handle, &mut bus_id) }, "读取 GPU 总线")?;
    checked(api, unsafe { slot(handle, &mut slot_id) }, "读取 GPU 插槽")?;
    let values = [pci_device, subsystem, revision, extended, bus_id, slot_id];
    if values.contains(&u32::MAX) {
        bail!("GPU PCI 身份未完整返回");
    }
    Ok(DeviceId {
        pci_device: values[0],
        subsystem: values[1],
        revision: values[2],
        extended: values[3],
        bus: values[4],
        slot: values[5],
    })
}

impl NvApiSession {
    fn identified_devices(&self) -> Result<Vec<(DeviceId, *mut c_void)>> {
        let mut result = Vec::new();
        for handle in self.api.gpu_handles()? {
            let id = identify(&self.api, handle)?;
            if result.iter().any(|(previous, _)| *previous == id) {
                bail!("NVIDIA 显卡 PCI 身份重复，不能确认目标");
            }
            result.push((id, handle));
        }
        Ok(result)
    }

    fn target(&self, id: &DeviceId) -> Result<*mut c_void> {
        self.identified_devices()?
            .into_iter()
            .find(|(current, _)| current == id)
            .map(|(_, handle)| handle)
            .ok_or_else(|| anyhow!("GPU 身份已变化，原目标不可用"))
    }
}

impl TurboIo for NvApiSession {
    fn devices(&self) -> Result<Vec<DeviceId>> {
        Ok(self
            .identified_devices()?
            .into_iter()
            .map(|(id, _)| id)
            .collect())
    }

    fn primary(&self) -> Result<DeviceId> {
        identify(&self.api, self.handle)
    }

    fn base_clock(&self, id: &DeviceId) -> Result<u32> {
        let handle = self.target(id)?;
        let function = resolve!(self.api, clock::GET_FREQUENCIES, FrequenciesIo);
        let mut frequencies = clock::Frequencies::default();
        checked(
            &self.api,
            unsafe { function(handle, &mut frequencies) },
            "读取 GPU 基础频率",
        )?;
        frequencies.graphics_base_mhz()
    }

    fn locks(&self, id: &DeviceId) -> Result<clock::LockTable> {
        let handle = self.target(id)?;
        let function = resolve!(self.api, clock::GET_LOCKS, LockIo);
        let mut table = clock::LockTable::default();
        checked(
            &self.api,
            unsafe { function(handle, &mut table) },
            "读取 GPU 锁频",
        )?;
        table.owned()?;
        Ok(table)
    }

    fn write_locks(&self, id: &DeviceId, table: &clock::LockTable) -> Result<()> {
        table.owned()?;
        let handle = self.target(id)?;
        let function = resolve!(self.api, clock::SET_LOCKS, LockIo);
        let mut request = *table;
        checked(
            &self.api,
            unsafe { function(handle, &mut request) },
            "设置 GPU 锁频",
        )
    }

    fn offsets(&self, id: &DeviceId) -> Result<pstates::ParsedPstates> {
        let handle = self.target(id)?;
        Ok(pstates::parse(
            &self.api.read_pstates(handle).context("读取 GPU 偏移")?,
        ))
    }

    fn write_offsets(&self, id: &DeviceId, core: Option<i32>, memory: Option<i32>) -> Result<()> {
        let handle = self.target(id)?;
        self.api
            .write_pstates(handle, &pstates::build_set_request(core, memory))
    }
}

#[cfg(test)]
mod tests;
