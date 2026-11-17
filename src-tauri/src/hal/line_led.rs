//! 已核对的X170/X58灯条HID：分别为8297/CC及8911/CD，FF89/64B。
use super::model::Kind;
use anyhow::{Result, anyhow, bail};
use windows_sys::Win32::{
    Devices::{DeviceAndDriverInstallation::*, HumanInterfaceDevice::*},
    Foundation::*,
    Storage::FileSystem::*,
};

pub struct Device {
    handle: HANDLE,
    path: String,
    kind: Kind,
}
impl Drop for Device {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.handle);
        }
    }
}
struct Interfaces(HDEVINFO);
impl Drop for Interfaces {
    fn drop(&mut self) {
        unsafe {
            SetupDiDestroyDeviceInfoList(self.0);
        }
    }
}
impl Device {
    pub fn kind(&self) -> Kind {
        self.kind
    }
    pub fn identity(&self) -> String {
        self.path.clone()
    }
    pub fn open(kinds: &[Kind]) -> Result<Self> {
        let mut guid = unsafe { std::mem::zeroed() };
        unsafe {
            HidD_GetHidGuid(&mut guid);
        }
        let raw = unsafe {
            SetupDiGetClassDevsW(
                &guid,
                std::ptr::null(),
                std::ptr::null_mut(),
                DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
            )
        };
        if raw == -1isize {
            return Err(std::io::Error::last_os_error().into());
        }
        let list = Interfaces(raw);
        let mut matches = Vec::new();
        for index in 0..256 {
            let mut info: SP_DEVICE_INTERFACE_DATA = unsafe { std::mem::zeroed() };
            info.cbSize = std::mem::size_of_val(&info) as u32;
            if unsafe {
                SetupDiEnumDeviceInterfaces(list.0, std::ptr::null(), &guid, index, &mut info)
            } == 0
            {
                let error = std::io::Error::last_os_error();
                if error.raw_os_error() != Some(ERROR_NO_MORE_ITEMS as i32) {
                    return Err(error.into());
                }
                return unique(matches);
            }
            if let Some(device) = candidate(path(&list, &info)?, kinds)? {
                matches.push(device);
            }
        }
        bail!("HID接口超过枚举上限，未绑定灯条")
    }
    pub fn send(&self, report: &[u8; 64]) -> Result<()> {
        if report[0] != self.kind.report_id() {
            bail!("灯条报告ID无效");
        }
        if !unsafe { HidD_SetFeature(self.handle, report.as_ptr().cast(), 64) } {
            bail!("灯条发送失败：{}", std::io::Error::last_os_error());
        }
        Ok(())
    }
}
fn unique(mut devices: Vec<Device>) -> Result<Device> {
    if devices.len() != 1 {
        bail!(
            "需要唯一匹配的独立灯条，当前{}个，未绑定共享保存块",
            devices.len()
        );
    }
    Ok(devices.pop().expect("唯一设备"))
}
fn path(list: &Interfaces, info: &SP_DEVICE_INTERFACE_DATA) -> Result<Vec<u16>> {
    let mut needed = 0;
    unsafe {
        SetupDiGetDeviceInterfaceDetailW(
            list.0,
            info,
            std::ptr::null_mut(),
            0,
            &mut needed,
            std::ptr::null_mut(),
        );
    }
    if !(8..=65_536).contains(&needed) {
        bail!("灯条设备路径长度异常");
    }
    let mut storage = vec![0u64; (needed as usize).div_ceil(8)];
    let detail = storage
        .as_mut_ptr()
        .cast::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>();
    unsafe {
        (*detail).cbSize = std::mem::size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32;
    }
    if unsafe {
        SetupDiGetDeviceInterfaceDetailW(
            list.0,
            info,
            detail,
            needed,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    } == 0
    {
        return Err(std::io::Error::last_os_error().into());
    }
    let words = unsafe {
        std::slice::from_raw_parts((*detail).DevicePath.as_ptr(), (needed as usize - 4) / 2)
    };
    let end = words
        .iter()
        .position(|v| *v == 0)
        .ok_or_else(|| anyhow!("灯条路径缺少终止符"))?;
    Ok(words[..=end].to_vec())
}
fn candidate(path: Vec<u16>, kinds: &[Kind]) -> Result<Option<Device>> {
    let name = String::from_utf16(&path[..path.len() - 1])?;
    let Some(kind) = kinds.iter().copied().find(|kind| {
        name.to_ascii_lowercase()
            .contains(&format!("vid_048d&pid_{:04x}", kind.product()))
    }) else {
        return Ok(None);
    };
    let handle = unsafe {
        CreateFileW(
            path.as_ptr(),
            GENERIC_READ | GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            std::ptr::null(),
            OPEN_EXISTING,
            0,
            std::ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(std::io::Error::last_os_error().into());
    }
    let device = Device {
        handle,
        path: name,
        kind,
    };
    let mut data: PHIDP_PREPARSED_DATA = 0;
    if !unsafe { HidD_GetPreparsedData(handle, &mut data) } {
        return Err(std::io::Error::last_os_error().into());
    }
    let mut caps: HIDP_CAPS = unsafe { std::mem::zeroed() };
    let status = unsafe { HidP_GetCaps(data, &mut caps) };
    unsafe {
        HidD_FreePreparsedData(data);
    }
    if status != HIDP_STATUS_SUCCESS {
        bail!("灯条HID能力读取失败：{status:#x}");
    }
    let mut attrs: HIDD_ATTRIBUTES = unsafe { std::mem::zeroed() };
    attrs.Size = std::mem::size_of_val(&attrs) as u32;
    if !unsafe { HidD_GetAttributes(handle, &mut attrs) } {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok((attrs.VendorID == 0x048d
        && attrs.ProductID == kind.product()
        && caps.Usage == u16::from(kind.report_id())
        && caps.UsagePage == 0xff89
        && caps.FeatureReportByteLength == 64)
        .then_some(device))
}
