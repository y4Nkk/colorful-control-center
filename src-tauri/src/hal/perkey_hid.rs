//! 静态核对 perkey_api.dll：usage=CC, usagePage=FF89, PID=8910；16B feature report。
use anyhow::{Result, anyhow, bail};
use windows_sys::Win32::{
    Devices::{DeviceAndDriverInstallation::*, HumanInterfaceDevice::*},
    Foundation::*,
    Storage::FileSystem::*,
};
pub struct Keyboard {
    handle: HANDLE,
    path: String,
}
impl Drop for Keyboard {
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
pub fn report(command: [u8; 5]) -> [u8; 16] {
    let mut bytes = [0u8; 16];
    bytes[0] = 0xCC;
    bytes[1..6].copy_from_slice(&command);
    bytes
}
impl Keyboard {
    pub fn identity(&self) -> String {
        self.path.clone()
    }
    pub fn open() -> Result<Self> {
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
            bail!("枚举逐键 HID 失败");
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
                break;
            }
            if let Some(keyboard) = open_interface(&list, &info)? {
                matches.push(keyboard);
            }
        }
        if matches.len() != 1 {
            bail!("需要唯一匹配的逐键 HID，当前 {} 个", matches.len());
        }
        Ok(matches.pop().expect("唯一设备"))
    }
    pub fn send(&self, command: [u8; 5]) -> Result<()> {
        let bytes = report(command);
        if !unsafe { HidD_SetFeature(self.handle, bytes.as_ptr().cast(), bytes.len() as u32) } {
            bail!("逐键 HID 写入失败：{}", std::io::Error::last_os_error());
        }
        Ok(())
    }
    pub fn query(&self, selector: u8) -> Result<[u8; 3]> {
        let request = super::device::query(selector)?;
        let mut response = [0u8; 16];
        response[0] = 0xCC;
        if !unsafe { HidD_SetFeature(self.handle, request.as_ptr().cast(), request.len() as u32) }
            || !unsafe {
                HidD_GetFeature(
                    self.handle,
                    response.as_mut_ptr().cast(),
                    response.len() as u32,
                )
            }
        {
            bail!("逐键MCU查询失败：{}", std::io::Error::last_os_error());
        }
        super::device::payload(&response)
    }
}
fn open_interface(list: &Interfaces, info: &SP_DEVICE_INTERFACE_DATA) -> Result<Option<Keyboard>> {
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
    if !(8..=65536).contains(&needed) {
        bail!("HID 设备路径长度异常");
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
        return Err(anyhow!("读取 HID 路径失败"));
    }
    let path = unsafe {
        std::slice::from_raw_parts((*detail).DevicePath.as_ptr(), (needed as usize - 4) / 2)
    };
    let end = path
        .iter()
        .position(|v| *v == 0)
        .ok_or_else(|| anyhow!("HID 路径没有终止符"))?;
    let path_string = String::from_utf16_lossy(&path[..end]);
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
        return Ok(None);
    }
    let keyboard = Keyboard {
        handle,
        path: path_string,
    };
    let mut data: PHIDP_PREPARSED_DATA = 0;
    if !unsafe { HidD_GetPreparsedData(handle, &mut data) } {
        return Ok(None);
    }
    let mut caps: HIDP_CAPS = unsafe { std::mem::zeroed() };
    let status = unsafe { HidP_GetCaps(data, &mut caps) };
    unsafe {
        HidD_FreePreparsedData(data);
    }
    let mut attrs: HIDD_ATTRIBUTES = unsafe { std::mem::zeroed() };
    attrs.Size = std::mem::size_of_val(&attrs) as u32;
    let matched = status == HIDP_STATUS_SUCCESS
        && caps.Usage == 0xCC
        && caps.UsagePage == 0xFF89
        && caps.FeatureReportByteLength == 16
        && unsafe { HidD_GetAttributes(handle, &mut attrs) }
        && attrs.ProductID == 0x8910;
    Ok(matched.then_some(keyboard))
}
