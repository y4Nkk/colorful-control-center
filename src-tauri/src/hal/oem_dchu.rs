//! InsydeDCHU.dll FFI（T1 通道，契约：docs/oem-dchu-contract.md）。
//! 读路径无条件开放；写路径仅供 fanctl / powerctl 受控调用，必须过
//! ccc-core 的校验与钳位（安全设计 docs/design.md §4）。

use anyhow::{anyhow, Result};
use windows_sys::Win32::Foundation::{FreeLibrary, FARPROC};
use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};

const DLL_PATH: &str = "C:\\Program Files (x86)\\ControlCenter\\DCHU\\InsydeDCHU.dll";
const STATUS_UNSUPPORTED: i32 = i32::MIN + 2; // 0x80000002

type GetInt = unsafe extern "system" fn(command: i32, data: *mut i32) -> i32;
type GetBuf = unsafe extern "system" fn(command: i32, buffer: *mut u8) -> i32;
type ReadSettings =
    unsafe extern "system" fn(page: i32, offset: i32, length: i32, buffer: *mut u8) -> i32;
type SetData = unsafe extern "system" fn(command: i32, buffer: *const u8, length: i32) -> i32;
type WriteSettings =
    unsafe extern "system" fn(page: i32, offset: i32, length: i32, buffer: *const u8) -> i32;

pub struct Dchu {
    handle: *mut core::ffi::c_void,
    get_int: GetInt,
    get_buf: GetBuf,
    read_settings: ReadSettings,
    set_data: SetData,
    write_settings: WriteSettings,
}

unsafe impl Send for Dchu {}
unsafe impl Sync for Dchu {}

impl Dchu {
    pub fn load() -> Result<Self> {
        let mut wide: Vec<u16> = DLL_PATH.encode_utf16().collect();
        wide.push(0);
        unsafe {
            let handle = LoadLibraryW(wide.as_ptr());
            if handle.is_null() {
                return Err(anyhow!("{} 加载失败（驱动未运行或 OEM CC 已卸载）", DLL_PATH));
            }
            let to_ptr = |p: FARPROC| -> *mut core::ffi::c_void {
                std::mem::transmute::<FARPROC, *mut core::ffi::c_void>(p)
            };
            let get_int_ptr = to_ptr(GetProcAddress(handle, b"GetDCHU_Data_Integer\0".as_ptr()));
            let get_buf_ptr = to_ptr(GetProcAddress(handle, b"GetDCHU_Data_Buffer\0".as_ptr()));
            let read_ptr = to_ptr(GetProcAddress(handle, b"ReadAppSettings\0".as_ptr()));
            let set_ptr = to_ptr(GetProcAddress(handle, b"SetDCHU_Data\0".as_ptr()));
            let write_ptr = to_ptr(GetProcAddress(handle, b"WriteAppSettings\0".as_ptr()));
            if get_int_ptr.is_null()
                || get_buf_ptr.is_null()
                || read_ptr.is_null()
                || set_ptr.is_null()
                || write_ptr.is_null()
            {
                FreeLibrary(handle);
                return Err(anyhow!("InsydeDCHU.dll 缺少必需导出"));
            }
            Ok(Self {
                handle,
                get_int: std::mem::transmute(get_int_ptr),
                get_buf: std::mem::transmute(get_buf_ptr),
                read_settings: std::mem::transmute(read_ptr),
                set_data: std::mem::transmute(set_ptr),
                write_settings: std::mem::transmute(write_ptr),
            })
        }
    }

    /// 读整数命令。返回 (ok, value)。
    /// ⚠️ 真机验证（2026-11-12）：该导出在非管理/管理下均返回静态回声
    /// （值==命令号、负载下不变）——采样路径禁止使用，仅供诊断对比。
    #[allow(dead_code)]
    pub fn read_int(&self, command: i32) -> (bool, i32) {
        let mut value: i32 = 0;
        unsafe {
            let status = (self.get_int)(command, &mut value);
            let ok = status != STATUS_UNSUPPORTED && status != -1;
            (ok, value)
        }
    }

    pub fn read_package(&self, command: i32) -> [u8; 256] {
        let mut buffer = [0u8; 256];
        unsafe {
            (self.get_buf)(command, buffer.as_mut_ptr());
        }
        buffer
    }

    #[allow(dead_code)]
    pub fn read_settings(&self, page: i32, offset: i32, length: usize) -> Vec<u8> {
        let mut buffer = vec![0u8; length];
        unsafe {
            (self.read_settings)(page, offset, length as i32, buffer.as_mut_ptr());
        }
        buffer
    }

    /// SetWMI(cmd, sub, value)：4 字节 LE，最高字节（buf[3]）= sub。
    /// 返回 DLL 状态（0 = 成功；0x80000002 不支持）。调用方负责校验。
    pub fn set_data_sub(&self, command: i32, sub: u8, value: u32) -> Result<()> {
        let mut buffer = value.to_le_bytes();
        buffer[3] = sub;
        unsafe {
            let status = (self.set_data)(command, buffer.as_ptr(), 4);
            if status != 0 {
                return Err(anyhow!("SetDCHU(cmd={command}, sub={sub}) 失败：status={status:#x}"));
            }
        }
        Ok(())
    }

    /// SetWMIPackage：256 字节整包写。
    pub fn set_package(&self, command: i32, package: &[u8; 256]) -> Result<()> {
        unsafe {
            let status = (self.set_data)(command, package.as_ptr(), 256);
            if status != 0 {
                return Err(anyhow!("SetWMIPackage(cmd={command}) 失败：status={status:#x}"));
            }
        }
        Ok(())
    }

    /// WriteAppSettings(page, offset, data)。
    pub fn write_settings(&self, page: i32, offset: i32, data: &[u8]) -> Result<()> {
        unsafe {
            let status = (self.write_settings)(page, offset, data.len() as i32, data.as_ptr());
            if status != 0 {
                return Err(anyhow!(
                    "WriteAppSettings(page={page}, offset={offset}) 失败：status={status:#x}"
                ));
            }
        }
        Ok(())
    }
}

impl Drop for Dchu {
    fn drop(&mut self) {
        unsafe { FreeLibrary(self.handle) };
    }
}
