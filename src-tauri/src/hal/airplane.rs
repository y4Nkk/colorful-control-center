//! InsydeAirplanemodehid.dll 的绝对状态协议；不加载原厂 DLL。
use crate::airplane::Snapshot;
use anyhow::{Context, Result, bail};
use std::{ffi::c_void, ptr};
use windows_sys::{
    Win32::{
        Devices::DeviceAndDriverInstallation::{
            CM_Get_Device_Interface_List_SizeW, CM_Get_Device_Interface_ListW, CR_SUCCESS,
        },
        Foundation::{CloseHandle, INVALID_HANDLE_VALUE},
        Storage::FileSystem::{
            CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
        },
        System::{
            IO::DeviceIoControl,
            Registry::{HKEY_LOCAL_MACHINE, RRF_RT_REG_DWORD, RegGetValueW},
        },
    },
    core::GUID,
};

const INTERFACE: GUID = GUID::from_u128(0x6e59105f_34e7_4b3c_b676_c5c8233c008d);
const IOCTL: u32 = 0x00322804;
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}
fn device() -> Result<String> {
    let mut size = 0;
    let status =
        unsafe { CM_Get_Device_Interface_List_SizeW(&mut size, &INTERFACE, ptr::null(), 0) };
    if status == CR_SUCCESS && size == 1 {
        bail!("Windows 未发现原厂飞行模式设备接口");
    }
    if status != CR_SUCCESS || !(2..=65536).contains(&size) {
        bail!("飞行模式设备列表长度无效：CONFIGRET={status:#x}, size={size}");
    }
    let mut buffer = vec![0u16; size as usize];
    let status = unsafe {
        CM_Get_Device_Interface_ListW(&INTERFACE, ptr::null(), buffer.as_mut_ptr(), size, 0)
    };
    if status != CR_SUCCESS {
        bail!("读取飞行模式设备失败：CONFIGRET={status:#x}");
    }
    decode_device(&buffer)
}
fn decode_device(buffer: &[u16]) -> Result<String> {
    if buffer.len() < 2 || buffer[buffer.len() - 2..] != [0, 0] {
        bail!("飞行模式设备列表缺少结束符");
    }
    let paths: Vec<_> = buffer[..buffer.len() - 1]
        .split(|x| *x == 0)
        .filter(|s| !s.is_empty())
        .collect();
    if paths.len() != 1 {
        bail!("飞行模式设备数量应为1，实际为{}", paths.len());
    }
    let path = String::from_utf16(paths[0])?;
    if !path.starts_with("\\\\?\\") || path.len() > 4096 {
        bail!("飞行模式设备路径无效");
    }
    Ok(path)
}
fn radio_state() -> Result<bool> {
    let key = wide(r"SYSTEM\CurrentControlSet\Control\RadioManagement\SystemRadioState");
    let (mut value, mut size) = (0u32, 4u32);
    let status = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            key.as_ptr(),
            ptr::null(),
            RRF_RT_REG_DWORD,
            ptr::null_mut(),
            &mut value as *mut _ as *mut c_void,
            &mut size,
        )
    };
    if status != 0 {
        bail!("读取系统飞行模式失败：WIN32={status}");
    }
    if size != 4 || value > 1 {
        bail!("系统飞行模式报告无效：size={size}, value={value}");
    }
    Ok(value == 1)
}
pub(crate) fn read() -> Result<Snapshot> {
    Ok(Snapshot {
        device: device()?,
        enabled: radio_state()?,
    })
}
pub(crate) fn require_device(expected: &str) -> Result<()> {
    if device()? != expected {
        bail!("飞行模式设备在写入前变化");
    }
    Ok(())
}
struct Handle(windows_sys::Win32::Foundation::HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}
pub(crate) fn write(expected: &str, enabled: bool) -> Result<()> {
    require_device(expected)?;
    let path = wide(expected);
    let handle = unsafe {
        CreateFileW(
            path.as_ptr(),
            0xc0000000,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            ptr::null(),
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(std::io::Error::last_os_error()).context("打开原厂飞行模式设备");
    }
    let handle = Handle(handle);
    let byte = u8::from(enabled);
    let mut returned = 0;
    let ok = unsafe {
        DeviceIoControl(
            handle.0,
            IOCTL,
            &byte as *const _ as *const c_void,
            1,
            ptr::null_mut(),
            0,
            &mut returned,
            ptr::null_mut(),
        )
    };
    if ok == 0 {
        return Err(std::io::Error::last_os_error()).context("设置原厂飞行模式");
    }
    if returned != 0 {
        bail!("飞行模式设备返回了未声明输出：{returned}字节");
    }
    Ok(())
}
