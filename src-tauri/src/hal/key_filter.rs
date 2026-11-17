//! 原厂HKFltrLib.h的cdecl边界；本模块不读取配置或决定恢复基线。
#[cfg(test)]
mod tests;
mod transaction;
use anyhow::{Result, anyhow, bail};
use ccc_core::keymap::{Configuration, Device};
use std::ffi::c_void;
use windows_sys::Win32::Foundation::FreeLibrary;
use windows_sys::Win32::System::LibraryLoader::{
    GetProcAddress, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR, LOAD_LIBRARY_SEARCH_SYSTEM32, LoadLibraryExW,
};

pub(crate) const DLL_PATH: &str = r"C:\Program Files (x86)\ControlCenter\Driver\x64\HKFltrLib.dll";
type Handle = *mut c_void;
type Create = unsafe extern "C" fn() -> Handle;
type Release = unsafe extern "C" fn(Handle) -> i32;
type Open = unsafe extern "C" fn(i32) -> Handle;
type Enable = unsafe extern "C" fn(Handle, i32) -> i32;
type AddKey = unsafe extern "C" fn(Handle, i32, u16, u32, u32) -> i32;
type AddEdge = unsafe extern "C" fn(Handle, u16, u32) -> i32;
type AddText = unsafe extern "C" fn(Handle, *const u16, u32, u32) -> i32;
type AddMacro = unsafe extern "C" fn(Handle, Handle, Handle) -> i32;
type GetError = unsafe extern "C" fn() -> i32;

struct Api {
    create: Create,
    delete: Release,
    open: Open,
    close: Release,
    enable: Enable,
    clear: Release,
    add_key: AddKey,
    down: AddEdge,
    up: AddEdge,
    text: AddText,
    register: AddMacro,
    error: GetError,
}
pub struct Library {
    handle: Handle,
    api: Api,
}
impl Drop for Library {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe {
                FreeLibrary(self.handle);
            }
        }
    }
}
fn checked(operation: &str, status: i32) -> Result<()> {
    let reason = match status {
        0 => return Ok(()),
        1 => "操作失败",
        2 => "内存不足",
        3 => "按键类型无效",
        4 => "按键或文本无效",
        5 => "集合句柄无效",
        6 => "设备类型无效",
        7 => "设备句柄无效",
        8 => "原厂剪贴板服务不存在",
        9 => "过滤设备不存在",
        10 => "参数无效",
        _ => "未知原厂错误",
    };
    bail!("{operation}：{reason}（HKFltrLib={status}）")
}
struct Module(Handle);
impl Drop for Module {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                FreeLibrary(self.0);
            }
        }
    }
}
impl Module {
    fn symbol<T: Copy>(&self, name: &'static [u8]) -> Result<T> {
        let pointer = unsafe { GetProcAddress(self.0, name.as_ptr()) }.ok_or_else(|| {
            anyhow!(
                "原厂过滤库缺少{}",
                String::from_utf8_lossy(&name[..name.len() - 1])
            )
        })?;
        // 每个类型都来自同一原厂头文件；x64指针宽度在赋值前校验。
        if std::mem::size_of::<T>() != std::mem::size_of_val(&pointer) {
            bail!("过滤库函数指针宽度异常");
        }
        Ok(unsafe { std::mem::transmute_copy(&pointer) })
    }
}
impl Library {
    pub fn load() -> Result<Self> {
        let path: Vec<u16> = DLL_PATH.encode_utf16().chain([0]).collect();
        let module = Module(unsafe {
            LoadLibraryExW(
                path.as_ptr(),
                std::ptr::null_mut(),
                LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32,
            )
        });
        if module.0.is_null() {
            bail!("无法加载原厂过滤库：{}", std::io::Error::last_os_error());
        }
        let api = Api {
            create: module.symbol(b"HKFltrLib_CreateKeyCollection\0")?,
            delete: module.symbol(b"HKFltrLib_DeleteKeyCollection\0")?,
            open: module.symbol(b"HKFltrLib_OpenDevice\0")?,
            close: module.symbol(b"HKFltrLib_CloseDevice\0")?,
            enable: module.symbol(b"HKFltrLib_EnableFilter\0")?,
            clear: module.symbol(b"HKFltrLib_ClearFilterMacro\0")?,
            add_key: module.symbol(b"HKFltrLib_AddKeyCode\0")?,
            down: module.symbol(b"HKFltrLib_AddMacroDown\0")?,
            up: module.symbol(b"HKFltrLib_AddMacroUp\0")?,
            text: module.symbol(b"HKFltrLib_AddKeyString\0")?,
            register: module.symbol(b"HKFltrLib_AddFilterMacro\0")?,
            error: module.symbol(b"HKFltrLib_GetError\0")?,
        };
        let handle = module.0;
        std::mem::forget(module);
        Ok(Self { handle, api })
    }
    pub fn replace(&self, config: &Configuration) -> Result<()> {
        let plan = ccc_core::keymap::compile::configuration(config).map_err(|e| anyhow!(e))?;
        transaction::replace(&self.api, &plan)
    }
}
fn device_code(device: Device) -> i32 {
    match device {
        Device::Keyboard => 0,
        Device::Mouse => 1,
    }
}
