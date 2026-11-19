//! InsydeDCHU.dll FFI（T1 通道，契约：docs/oem-dchu-contract.md）。
//! 读路径无条件开放；写路径仅供 fanctl / powerctl 受控调用，必须过
//! ccc-core 的校验与钳位（安全设计 docs/design.md §4）。

use anyhow::{Result, anyhow};
use std::sync::atomic::AtomicU32;
use windows_sys::Win32::Foundation::{FARPROC, FreeLibrary};
use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};

const DLL_PATH: &str = "C:\\Program Files (x86)\\ControlCenter\\DCHU\\InsydeDCHU.dll";
const OUTPUT_CAPACITY: usize = 1025; // 0x40C ACPI 输出容器，含 DLL 的尾随复制。
#[path = "oem_dchu/capabilities.rs"]
mod capabilities;

type GetInt = unsafe extern "system" fn(command: i32, data: *mut i32) -> i32;
type GetBuf = unsafe extern "system" fn(command: i32, buffer: *mut u8) -> i32;
type ReadSettings =
    unsafe extern "system" fn(page: i32, offset: i32, length: i32, buffer: *mut u8) -> i32;
type SetData = unsafe extern "system" fn(command: i32, buffer: *const u8, length: i32) -> i32;
type SetDataEx =
    unsafe extern "system" fn(command: i32, buffer: *const u8, length: i32, output: *mut u8) -> i32;
type WriteSettings =
    unsafe extern "system" fn(page: i32, offset: i32, length: i32, buffer: *const u8) -> i32;

pub struct Dchu {
    handle: *mut core::ffi::c_void,
    get_int: GetInt,
    get_buf: GetBuf,
    read_settings: ReadSettings,
    set_data: SetData,
    set_data_ex: Option<SetDataEx>,
    write_settings: WriteSettings,
    pub(crate) acknowledged_mode: AtomicU32,
    #[cfg(test)]
    pub(crate) test_entertainment_options: std::sync::atomic::AtomicBool,
}

unsafe impl Send for Dchu {}
unsafe impl Sync for Dchu {}

impl Dchu {
    #[cfg(test)]
    pub(crate) fn test_io(
        get_buf: GetBuf,
        read_settings: ReadSettings,
        set_data: SetData,
        write_settings: WriteSettings,
    ) -> Self {
        unsafe extern "system" fn no_integer(_: i32, _: *mut i32) -> i32 {
            0
        }
        Self {
            handle: std::ptr::null_mut(),
            get_int: no_integer,
            get_buf,
            read_settings,
            set_data,
            set_data_ex: None,
            write_settings,
            acknowledged_mode: AtomicU32::new(0),
            test_entertainment_options: std::sync::atomic::AtomicBool::new(false),
        }
    }

    #[cfg(test)]
    pub(crate) fn with_test_exchange(mut self, exchange: SetDataEx) -> Self {
        self.set_data_ex = Some(exchange);
        self
    }

    #[cfg(test)]
    pub(crate) fn with_test_integer(mut self, integer: GetInt) -> Self {
        self.get_int = integer;
        self
    }

    pub fn load() -> Result<Self> {
        let mut wide: Vec<u16> = DLL_PATH.encode_utf16().collect();
        wide.push(0);
        unsafe {
            let handle = LoadLibraryW(wide.as_ptr());
            if handle.is_null() {
                return Err(anyhow!(
                    "{} 加载失败（驱动未运行或 OEM CC 已卸载）",
                    DLL_PATH
                ));
            }
            let to_ptr = |p: FARPROC| -> *mut core::ffi::c_void {
                std::mem::transmute::<FARPROC, *mut core::ffi::c_void>(p)
            };
            let get_int_ptr = to_ptr(GetProcAddress(handle, b"GetDCHU_Data_Integer\0".as_ptr()));
            let get_buf_ptr = to_ptr(GetProcAddress(handle, b"GetDCHU_Data_Buffer\0".as_ptr()));
            let read_ptr = to_ptr(GetProcAddress(handle, b"ReadAppSettings\0".as_ptr()));
            let set_ptr = to_ptr(GetProcAddress(handle, b"SetDCHU_Data\0".as_ptr()));
            let set_ex_ptr = to_ptr(GetProcAddress(handle, b"SetDCHU_DataEx\0".as_ptr()));
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
                set_data_ex: if set_ex_ptr.is_null() {
                    None
                } else {
                    Some(std::mem::transmute(set_ex_ptr))
                },
                write_settings: std::mem::transmute(write_ptr),
                acknowledged_mode: AtomicU32::new(0),
                #[cfg(test)]
                test_entertainment_options: std::sync::atomic::AtomicBool::new(false),
            })
        }
    }

    /// 读整数命令。返回 (ok, value)。
    /// ⚠️ 真机验证（2026-11-12）：该导出在非管理/管理下均返回静态回声
    /// （值==命令号、负载下不变）。连续监控禁止使用；事件回读还须校验有效标志和领域范围。
    pub fn read_int(&self, command: i32) -> (bool, i32) {
        let mut value = i32::MIN;
        unsafe {
            let status = (self.get_int)(command, &mut value);
            // 此导出返回整数负载本身；失败的 0 不会改写输出。
            let ok = status == value && value != command && value != i32::MIN;
            (ok, value)
        }
    }

    pub fn read_package(&self, command: i32) -> Result<[u8; 256]> {
        let mut output = PacketOutput::new();
        let status = unsafe { (self.get_buf)(command, output.bytes.as_mut_ptr()) };
        output.finish(status, command, "GetDCHU_Data_Buffer")
    }

    pub fn exchange_package(&self, command: i32, package: &[u8; 256]) -> Result<[u8; 256]> {
        let exchange = self
            .set_data_ex
            .ok_or_else(|| anyhow!("OEM DLL 未提供双向包传输"))?;
        let mut output = PacketOutput::new();
        let status = unsafe { exchange(command, package.as_ptr(), 256, output.bytes.as_mut_ptr()) };
        output.finish(status, command, "SetDCHU_DataEx")
    }

    pub fn read_settings(&self, page: i32, offset: i32, length: usize) -> Result<Vec<u8>> {
        validate_settings_range(page, offset, length)?;
        if page == 7 {
            return Err(anyhow!("BIOS能力应通过实时查询读取，不读取page7缓存"));
        }
        let mut buffer = vec![0u8; length];
        let status =
            unsafe { (self.read_settings)(page, offset, length as i32, buffer.as_mut_ptr()) };
        check_status(
            status,
            4096,
            &format!("ReadAppSettings(page={page}, offset={offset})"),
        )?;
        Ok(buffer)
    }

    /// SetWMI(cmd, sub, value)：4 字节 LE，最高字节（buf[3]）= sub。
    /// 本机 DLL 成功返回固件命令号，0 表示传输失败。
    pub fn set_data_sub(&self, command: i32, sub: u8, value: u32) -> Result<()> {
        let mut buffer = value.to_le_bytes();
        buffer[3] = sub;
        let status = unsafe { (self.set_data)(command, buffer.as_ptr(), 4) };
        check_status(
            status,
            command,
            &format!("SetDCHU(cmd={command}, sub={sub})"),
        )
    }

    pub fn set_u32(&self, command: i32, value: u32) -> Result<()> {
        let bytes = value.to_le_bytes();
        let status = unsafe { (self.set_data)(command, bytes.as_ptr(), 4) };
        check_status(status, command, &format!("SetDCHU(cmd={command})"))
    }

    /// SetWMIPackage：256 字节整包写。
    pub fn set_package(&self, command: i32, package: &[u8; 256]) -> Result<()> {
        let status = unsafe { (self.set_data)(command, package.as_ptr(), 256) };
        check_status(status, command, &format!("SetWMIPackage(cmd={command})"))
    }

    /// WriteAppSettings(page, offset, data)。
    pub fn write_settings(&self, page: i32, offset: i32, data: &[u8]) -> Result<()> {
        validate_settings_range(page, offset, data.len())?;
        let status =
            unsafe { (self.write_settings)(page, offset, data.len() as i32, data.as_ptr()) };
        check_status(
            status,
            1,
            &format!("WriteAppSettings(page={page}, offset={offset})"),
        )
    }
}

impl Drop for Dchu {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe { FreeLibrary(self.handle) };
        }
    }
}

fn check_status(status: i32, expected: i32, operation: &str) -> Result<()> {
    if expected <= 0 || status != expected {
        return Err(anyhow!(
            "{operation} 失败：期望 {expected:#x}，status={status:#x}"
        ));
    }
    Ok(())
}

fn validate_settings_range(page: i32, offset: i32, length: usize) -> Result<()> {
    if !(0..=15).contains(&page)
        || offset < 0
        || length == 0
        || (offset as usize)
            .checked_add(length)
            .is_none_or(|end| end > 256)
    {
        return Err(anyhow!("AppSettings 页或范围无效"));
    }
    Ok(())
}

impl ccc_core::settings::SettingsAccess for Dchu {
    fn read(&self, page: u8, offset: usize, length: usize) -> std::result::Result<Vec<u8>, String> {
        let result = if page == 7 {
            self.read_bios_capabilities(offset, length)
        } else {
            self.read_settings(page as i32, offset as i32, length)
        };
        result.map_err(|e| format!("{e:#}"))
    }

    fn write(&self, page: u8, offset: usize, data: &[u8]) -> std::result::Result<(), String> {
        self.write_settings(page as i32, offset as i32, data)
            .map_err(|e| format!("{e:#}"))
    }
}

struct PacketOutput {
    bytes: [u8; OUTPUT_CAPACITY],
}
impl PacketOutput {
    fn new() -> Self {
        let mut bytes = [0xa5; OUTPUT_CAPACITY];
        bytes[256] = 0x5a;
        Self { bytes }
    }
    fn finish(self, status: i32, command: i32, operation: &str) -> Result<[u8; 256]> {
        check_status(status, command, operation)?;
        // 本机 DLL 在复制 256B 后重复首字节；不同初始值也能识别 Ex 的空回写。
        if self.bytes[256] != self.bytes[0] {
            return Err(anyhow!(
                "{operation}(cmd={command}) 未返回完整的 256 字节包"
            ));
        }
        Ok(self.bytes[..256].try_into().unwrap())
    }
}

#[cfg(test)]
#[path = "oem_dchu/tests.rs"]
mod tests;
