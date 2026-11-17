//! 只绑定当前存在的网络设备及其数值驱动子键，固定六字段读取、三字段写入。
use crate::platform::lan::{model::*, transaction::Io};
use anyhow::{Context, Result, bail};
use std::collections::BTreeMap;
use windows_sys::{
    Win32::{Devices::DeviceAndDriverInstallation::*, Foundation::*, System::Registry::*},
    core::GUID,
};
#[path = "network_policy/registry.rs"]
mod registry;
use registry::Key;

pub struct Native;
pub fn frame(dchu: &crate::hal::oem_dchu::Dchu, recovering: bool) -> Result<Frame> {
    let capabilities = Capabilities::decode(&dchu.read_bios_capabilities(0, 24)?)?;
    let recommendation = if capabilities.power_saving {
        let mut packet = [0; 256];
        packet[0] = 52;
        match dchu.exchange_package(4, &packet)?[0] {
            0 => None,
            1 => Some(true),
            2 => Some(false),
            _ => bail!("原厂LAN节能建议未知"),
        }
    } else {
        None
    };
    let mut devices = if capabilities.any() || recovering {
        devices()?
    } else {
        vec![]
    };
    devices.sort_by(|a, b| a.identity.instance.cmp(&b.identity.instance));
    let frame = Frame {
        capabilities,
        recommendation,
        devices,
    };
    frame.validate()?;
    Ok(frame)
}
pub(super) fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}
pub(super) fn checked(code: u32) -> Result<()> {
    if code != 0 {
        return Err(std::io::Error::from_raw_os_error(code as i32).into());
    }
    Ok(())
}
fn text(chars: &[u16]) -> Result<String> {
    let end = chars
        .iter()
        .position(|value| *value == 0)
        .context("LAN设备字符串未终止")?;
    Ok(String::from_utf16(&chars[..end])?)
}
struct Devices(HDEVINFO);
impl Drop for Devices {
    fn drop(&mut self) {
        unsafe {
            SetupDiDestroyDeviceInfoList(self.0);
        }
    }
}
fn property(set: &Devices, device: &SP_DEVINFO_DATA, field: u32) -> Result<String> {
    let mut bytes = [0u16; 1024];
    let (mut kind, mut size) = (0, 0);
    if unsafe {
        SetupDiGetDeviceRegistryPropertyW(
            set.0,
            device,
            field,
            &mut kind,
            bytes.as_mut_ptr().cast(),
            (bytes.len() * 2) as u32,
            &mut size,
        )
    } == 0
    {
        return Err(std::io::Error::last_os_error().into());
    }
    if kind != REG_SZ || size == 0 || size % 2 != 0 || size as usize > bytes.len() * 2 {
        bail!("LAN设备属性类型或长度无效");
    }
    text(&bytes[..size as usize / 2])
}
fn item(set: &Devices, info: &SP_DEVINFO_DATA) -> Result<Option<Device>> {
    let mut instance = [0u16; 512];
    if unsafe {
        SetupDiGetDeviceInstanceIdW(
            set.0,
            info,
            instance.as_mut_ptr(),
            instance.len() as u32,
            std::ptr::null_mut(),
        )
    } == 0
    {
        return Err(std::io::Error::last_os_error().into());
    }
    let driver = match property(set, info, SPDRP_DRIVER) {
        Ok(driver) => driver.to_ascii_lowercase(),
        Err(error)
            if error
                .downcast_ref::<std::io::Error>()
                .and_then(std::io::Error::raw_os_error)
                == Some(ERROR_INVALID_DATA as i32) =>
        {
            return Ok(None);
        }
        Err(error) => return Err(error),
    };
    let mut identity = Identity {
        instance: text(&instance)?.to_ascii_uppercase(),
        driver,
        net_cfg_id: None,
        driver_version: None,
        matching_id: None,
    };
    identity.validate()?;
    let key = Key::open(&identity.driver, false)?;
    (
        identity.net_cfg_id,
        identity.driver_version,
        identity.matching_id,
    ) = key.signature()?;
    let device = Device {
        identity,
        name: property(set, info, SPDRP_DEVICEDESC)?,
        values: FIELDS
            .into_iter()
            .map(|field| key.read(field.name()).map(|value| (field, value)))
            .collect::<Result<BTreeMap<_, _>>>()?,
    };
    device.validate()?;
    Ok(Some(device))
}
pub fn devices() -> Result<Vec<Device>> {
    let class = GUID::from_u128(0x4d36e972_e325_11ce_bfc1_08002be10318);
    let handle = unsafe {
        SetupDiGetClassDevsW(
            &class,
            std::ptr::null(),
            std::ptr::null_mut(),
            DIGCF_PRESENT,
        )
    };
    if handle == -1isize {
        return Err(std::io::Error::last_os_error().into());
    }
    let set = Devices(handle);
    let mut devices = Vec::new();
    for index in 0..256 {
        let mut info: SP_DEVINFO_DATA = unsafe { std::mem::zeroed() };
        info.cbSize = std::mem::size_of_val(&info) as u32;
        if unsafe { SetupDiEnumDeviceInfo(set.0, index, &mut info) } == 0 {
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() == Some(ERROR_NO_MORE_ITEMS as i32) {
                return Ok(devices);
            }
            return Err(error.into());
        }
        if let Some(device) = item(&set, &info)? {
            devices.push(device);
        }
    }
    bail!("LAN设备枚举超过256项，未发布不完整结果")
}
impl Io for Native {
    fn inspect(&self, instance: &str) -> Result<Device> {
        devices()?
            .into_iter()
            .find(|device| device.identity.instance == instance)
            .context("LAN设备已移除")
    }
    fn prepare(&self, _: &Device, _: &Change) -> Result<()> {
        bail!("LAN事务尚未绑定持久恢复记录，未写入系统")
    }
    fn write(&self, identity: &Identity, field: Field, value: &Option<Value>) -> Result<()> {
        identity.validate()?;
        crate::hal::oem_owner::ensure_owner_absent()?;
        if self.inspect(&identity.instance)?.identity != *identity {
            bail!("LAN设备实例或驱动绑定已变化");
        }
        if !field.writable() {
            bail!("LAN字段不允许写入");
        }
        if let Some(value) = value {
            value.validate()?;
        }
        let key = Key::open(&identity.driver, true)?;
        let signature = key.signature()?;
        if signature
            != (
                identity.net_cfg_id.clone(),
                identity.driver_version.clone(),
                identity.matching_id.clone(),
            )
        {
            bail!("LAN驱动签名已变化");
        }
        key.write(field, value)
    }
}
