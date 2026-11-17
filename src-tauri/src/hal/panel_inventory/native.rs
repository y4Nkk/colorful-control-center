use super::{Inventory, Monitor, decode_property};
use anyhow::{Result, bail};
use windows_sys::{
    Win32::{
        Devices::DeviceAndDriverInstallation::*,
        Foundation::{ERROR_NO_MORE_ITEMS, INVALID_HANDLE_VALUE},
        System::Registry::*,
    },
    core::GUID,
};
const MONITOR_CLASS: GUID = GUID::from_u128(0x4d36e96e_e325_11ce_bfc1_08002be10318);
struct Devices(HDEVINFO);
impl Drop for Devices {
    fn drop(&mut self) {
        unsafe {
            SetupDiDestroyDeviceInfoList(self.0);
        }
    }
}
struct Key(HKEY);
impl Drop for Key {
    fn drop(&mut self) {
        unsafe {
            RegCloseKey(self.0);
        }
    }
}
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain([0]).collect()
}

fn property(
    set: &Devices,
    device: &SP_DEVINFO_DATA,
    key: u32,
    expected: u32,
) -> Result<Vec<String>> {
    let mut buffer = [0u16; 4096];
    let (mut kind, mut required) = (0, 0);
    if unsafe {
        SetupDiGetDeviceRegistryPropertyW(
            set.0,
            device,
            key,
            &mut kind,
            buffer.as_mut_ptr().cast(),
            (buffer.len() * 2) as u32,
            &mut required,
        )
    } == 0
    {
        return Err(std::io::Error::last_os_error().into());
    }
    if kind != expected
        || required == 0
        || required as usize > buffer.len() * 2
        || required % 2 != 0
    {
        bail!("面板设备属性类型或长度无效");
    }
    let buffer = &buffer[..required as usize / 2];
    decode_property(buffer, expected == REG_MULTI_SZ)
}
fn edid(set: &Devices, device: &SP_DEVINFO_DATA) -> Result<Vec<u8>> {
    let raw =
        unsafe { SetupDiOpenDevRegKey(set.0, device, DICS_FLAG_GLOBAL, 0, DIREG_DEV, KEY_READ) };
    if raw.is_null() || raw == INVALID_HANDLE_VALUE {
        return Err(std::io::Error::last_os_error().into());
    }
    let key = Key(raw);
    let (mut kind, mut size) = (0, 0);
    let name = wide("EDID");
    let status = unsafe {
        RegQueryValueExW(
            key.0,
            name.as_ptr(),
            std::ptr::null(),
            &mut kind,
            std::ptr::null_mut(),
            &mut size,
        )
    };
    if status != 0 {
        return Err(std::io::Error::from_raw_os_error(status as i32).into());
    }
    if kind != REG_BINARY || !(128..=4096).contains(&size) {
        bail!("面板EDID属性类型或长度无效");
    }
    let mut bytes = vec![0u8; size as usize];
    let expected = size;
    let status = unsafe {
        RegQueryValueExW(
            key.0,
            name.as_ptr(),
            std::ptr::null(),
            &mut kind,
            bytes.as_mut_ptr(),
            &mut size,
        )
    };
    if status != 0 {
        return Err(std::io::Error::from_raw_os_error(status as i32).into());
    }
    if kind != REG_BINARY || size != expected {
        bail!("面板EDID在读取时发生变化");
    }
    Ok(bytes)
}
fn monitor(set: &Devices, device: &SP_DEVINFO_DATA) -> Result<Monitor> {
    let mut instance = [0u16; 4096];
    if unsafe {
        SetupDiGetDeviceInstanceIdW(
            set.0,
            device,
            instance.as_mut_ptr(),
            instance.len() as u32,
            std::ptr::null_mut(),
        )
    } == 0
    {
        return Err(std::io::Error::last_os_error().into());
    }
    let end = instance
        .iter()
        .position(|value| *value == 0)
        .ok_or_else(|| anyhow::anyhow!("面板实例标识未结束"))?;
    let instance = String::from_utf16(&instance[..end])?;
    let hardware = property(set, device, SPDRP_HARDWAREID, REG_MULTI_SZ)?;
    let description = property(set, device, SPDRP_DEVICEDESC, REG_SZ)?;
    let name = description
        .first()
        .ok_or_else(|| anyhow::anyhow!("面板设备描述为空"))?
        .clone();
    let (mut status, mut problem) = (0, 0);
    let result = unsafe { CM_Get_DevNode_Status(&mut status, &mut problem, device.DevInst, 0) };
    if result != CR_SUCCESS {
        bail!("面板设备节点状态不可读取：{result}");
    }
    Monitor::from_edid(
        instance,
        name,
        hardware,
        &edid(set, device)?,
        status,
        problem,
    )
}
pub(super) fn read() -> Result<Inventory> {
    let raw = unsafe {
        SetupDiGetClassDevsW(
            &MONITOR_CLASS,
            std::ptr::null(),
            std::ptr::null_mut(),
            DIGCF_PRESENT,
        )
    };
    if raw == INVALID_HANDLE_VALUE as isize {
        return Err(std::io::Error::last_os_error().into());
    }
    let set = Devices(raw);
    let mut inventory = Inventory {
        monitors: Vec::new(),
        unresolved: Vec::new(),
    };
    for index in 0..256 {
        let mut device: SP_DEVINFO_DATA = unsafe { std::mem::zeroed() };
        device.cbSize = std::mem::size_of_val(&device) as u32;
        if unsafe { SetupDiEnumDeviceInfo(set.0, index, &mut device) } == 0 {
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() == Some(ERROR_NO_MORE_ITEMS as i32) {
                return Ok(inventory);
            }
            return Err(error.into());
        }
        match monitor(&set, &device) {
            Ok(monitor) => inventory.monitors.push(monitor),
            Err(error) => inventory
                .unresolved
                .push(format!("面板节点{index}：{error:#}")),
        }
    }
    bail!("显示面板设备数量超出范围")
}
