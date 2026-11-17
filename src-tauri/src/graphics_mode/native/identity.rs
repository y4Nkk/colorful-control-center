//! 只读Windows安装和启动标识；不读取或写入UEFI变量。
use super::Identity;
use anyhow::{Result, bail};
use sha2::{Digest, Sha256};
use windows_sys::{
    Win32::System::{
        LibraryLoader::{GetModuleHandleW, GetProcAddress},
        Registry::*,
    },
    core::GUID,
};

type Query = unsafe extern "system" fn(i32, *mut core::ffi::c_void, u32, *mut u32) -> i32;
#[repr(C)]
struct BootInformation {
    identifier: GUID,
    firmware_type: u32,
    flags: u64,
}
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}
fn owner() -> Result<String> {
    let mut data = [0u16; 128];
    let mut size = (data.len() * 2) as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            wide("SOFTWARE\\Microsoft\\Cryptography").as_ptr(),
            wide("MachineGuid").as_ptr(),
            RRF_RT_REG_SZ | RRF_SUBKEY_WOW6464KEY,
            std::ptr::null_mut(),
            data.as_mut_ptr().cast(),
            &mut size,
        )
    };
    if status != 0 || size < 4 || size as usize > data.len() * 2 {
        bail!("读取Windows安装标识失败（{status}）");
    }
    let end = data
        .iter()
        .position(|c| *c == 0)
        .ok_or_else(|| anyhow::anyhow!("Windows安装标识未终止"))?;
    let text = String::from_utf16(&data[..end])?;
    if text.is_empty() {
        bail!("Windows安装标识为空");
    }
    Ok(format!(
        "{:x}",
        Sha256::digest(text.to_ascii_lowercase().as_bytes())
    ))
}
fn boot_with(query: Query) -> Result<String> {
    let mut info: BootInformation = unsafe { std::mem::zeroed() };
    let mut returned = 0;
    let length = std::mem::size_of_val(&info) as u32;
    let status = unsafe {
        query(
            90,
            (&mut info as *mut BootInformation).cast(),
            length,
            &mut returned,
        )
    };
    if status != 0 || returned != length {
        bail!(
            "无法读取可靠的系统启动标识（状态0x{:08X}，长度{returned}）",
            status as u32
        );
    }
    let bytes =
        unsafe { std::slice::from_raw_parts((&info.identifier as *const GUID).cast::<u8>(), 16) };
    if bytes.iter().all(|b| *b == 0) {
        bail!("系统启动标识为空");
    }
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}
fn boot() -> Result<String> {
    let module = unsafe { GetModuleHandleW(wide("ntdll.dll").as_ptr()) };
    if module.is_null() {
        bail!("Windows未提供系统启动标识接口");
    }
    let symbol = unsafe { GetProcAddress(module, b"NtQuerySystemInformation\0".as_ptr()) };
    let Some(symbol) = symbol else {
        bail!("Windows未提供系统启动标识查询");
    };
    boot_with(unsafe { std::mem::transmute::<unsafe extern "system" fn() -> isize, Query>(symbol) })
}
pub fn read() -> Result<Identity> {
    Ok(Identity {
        owner: owner()?,
        boot: boot()?,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    unsafe extern "system" fn good(
        class: i32,
        output: *mut core::ffi::c_void,
        length: u32,
        returned: *mut u32,
    ) -> i32 {
        assert_eq!(class, 90);
        assert_eq!(length, 32);
        let info = BootInformation {
            identifier: GUID::from_u128(0x12345678_1234_5678_1122_334455667788),
            firmware_type: 2,
            flags: 0,
        };
        unsafe {
            output.cast::<BootInformation>().write(info);
            returned.write(length);
        }
        0
    }
    unsafe extern "system" fn denied(
        _: i32,
        _: *mut core::ffi::c_void,
        _: u32,
        _: *mut u32,
    ) -> i32 {
        0xc0000022u32 as i32
    }
    unsafe extern "system" fn empty(
        _: i32,
        _: *mut core::ffi::c_void,
        length: u32,
        returned: *mut u32,
    ) -> i32 {
        unsafe {
            returned.write(length);
        }
        0
    }
    #[test]
    fn exact_boot_abi_accepts_only_successful_nonempty_identity() {
        assert_eq!(std::mem::size_of::<BootInformation>(), 32);
        assert_eq!(boot_with(good).unwrap().len(), 32);
        assert!(boot_with(denied).is_err());
        assert!(boot_with(empty).is_err());
    }
}
