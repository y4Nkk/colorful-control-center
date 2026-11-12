//! InsydeDCHU.dll 只读 FFI（probe-cli 独立副本；应用内 HAL 见 src-tauri）。
//! 契约：docs/oem-dchu-contract.md。本模块只读，禁止封装任何写入导出。

#![allow(non_snake_case)]

use anyhow::{anyhow, Result};
use windows_sys::Win32::Foundation::{CloseHandle, FreeLibrary, FARPROC};
use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};

const DLL_PATH: &str = "C:\\Program Files (x86)\\ControlCenter\\DCHU\\InsydeDCHU.dll";
pub const STATUS_UNSUPPORTED: i32 = i32::MIN + 2; // 0x80000002

type GetDCHUDataInteger = unsafe extern "system" fn(command: i32, data: *mut i32) -> i32;
type GetDCHUDataBuffer = unsafe extern "system" fn(command: i32, buffer: *mut u8) -> i32;
type ReadAppSettings =
    unsafe extern "system" fn(page: i32, offset: i32, length: i32, buffer: *mut u8) -> i32;

pub struct Dchu {
    handle: *mut core::ffi::c_void,
    get_int: GetDCHUDataInteger,
    get_buf: GetDCHUDataBuffer,
    read_settings: ReadAppSettings,
}

unsafe impl Send for Dchu {}
unsafe impl Sync for Dchu {}

impl Dchu {
    /// 加载 OEM DLL（依赖已安装的 AcpiBridge 驱动）。失败返回可读错误。
    pub fn load() -> Result<Self> {
        let mut wide: Vec<u16> = DLL_PATH.encode_utf16().collect();
        wide.push(0);
        unsafe {
            let handle = LoadLibraryW(wide.as_ptr());
            if handle.is_null() {
                return Err(anyhow!(
                    "InsydeDCHU.dll 加载失败：{}（OEM Control Center 未安装或驱动未运行）",
                    DLL_PATH
                ));
            }
            // FARPROC = Option<fn>；先统一转成裸指针做空检查，再按各导出签名转型。
            let to_ptr = |p: FARPROC| -> *mut core::ffi::c_void {
                std::mem::transmute::<FARPROC, *mut core::ffi::c_void>(p)
            };
            let get_int_ptr = to_ptr(GetProcAddress(handle, b"GetDCHU_Data_Integer\0".as_ptr()));
            let get_buf_ptr = to_ptr(GetProcAddress(handle, b"GetDCHU_Data_Buffer\0".as_ptr()));
            let read_ptr = to_ptr(GetProcAddress(handle, b"ReadAppSettings\0".as_ptr()));
            if get_int_ptr.is_null() || get_buf_ptr.is_null() || read_ptr.is_null() {
                FreeLibrary(handle);
                return Err(anyhow!("InsydeDCHU.dll 缺少必需导出"));
            }
            Ok(Self {
                handle,
                get_int: std::mem::transmute(get_int_ptr),
                get_buf: std::mem::transmute(get_buf_ptr),
                read_settings: std::mem::transmute(read_ptr),
            })
        }
    }

    /// 读整数命令。Ok=false 表示不支持/失败（0x80000002 / -1）。
    pub fn read_int(&self, command: i32) -> (bool, i32) {
        let mut value: i32 = 0;
        unsafe {
            let status = (self.get_int)(command, &mut value);
            let ok = status != STATUS_UNSUPPORTED && status != -1;
            (ok, value)
        }
    }

    /// 读 256B 包。
    pub fn read_package(&self, command: i32) -> [u8; 256] {
        let mut buffer = [0u8; 256];
        unsafe {
            (self.get_buf)(command, buffer.as_mut_ptr());
        }
        buffer
    }

    /// 读 AppSettings（page, offset, length）。
    pub fn read_settings(&self, page: i32, offset: i32, length: usize) -> Vec<u8> {
        let mut buffer = vec![0u8; length];
        unsafe {
            (self.read_settings)(page, offset, length as i32, buffer.as_mut_ptr());
        }
        buffer
    }

    /// 读 AppSettings 的扁平 index（index = page*256 + offset）。
    pub fn read_settings_index(&self, index: i32, length: usize) -> Vec<u8> {
        self.read_settings(index / 256, index % 256, length)
    }
}

impl Drop for Dchu {
    fn drop(&mut self) {
        unsafe { FreeLibrary(self.handle) };
    }
}

fn u16_le(buf: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([buf[at], buf[at + 1]])
}
fn u32_le(buf: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([buf[at], buf[at + 1], buf[at + 2], buf[at + 3]])
}

/// 能力位图①（cmd 16）解码。
pub fn decode_cmd16(v: i32) -> Vec<&'static str> {
    let v = v as u32;
    let mut out = vec![];
    for (bit, name) in [
        (0, "PowerMode"),
        (1, "Flexikey"),
        (2, "FlexiAccess"),
        (3, "PerKeyKB"),
        (4, "RGBKB"),
        (5, "GPUOC"),
        (6, "CPUOC"),
        (7, "FanSetting"),
    ] {
        if (v >> bit) & 1 == 1 {
            out.push(name);
        }
    }
    out
}

/// 输出 Phase 0 能力结论报告（纯只读）。
pub fn report() -> Result<()> {
    let dchu = Dchu::load()?;
    println!("== InsydeDCHU 能力报告（只读）==");

    let (ok16, cmd16) = dchu.read_int(16);
    println!(
        "cmd16 特性位图: {:?} -> {}",
        cmd16,
        if ok16 {
            decode_cmd16(cmd16).join(", ")
        } else {
            "读取失败".to_string()
        }
    );
    let cpuoc = ok16 && (cmd16 as u32 >> 6) & 1 == 1;

    let (ok122, cmd122) = dchu.read_int(122);
    let cpuoc_wmi122 = ok122 && (cmd122 as u32 >> 23) & 1 == 1;
    println!(
        "cmd122: bit15 FanLess={} bit23 CPUOC={}",
        ok122 && ((cmd122 as u32 >> 15) & 1) == 1,
        cpuoc_wmi122
    );

    let (ok96, cmd96) = dchu.read_int(96);
    println!(
        "cmd96: bit7 AntiDust={} bit10 FanOffset={}",
        ok96 && ((cmd96 as u32 >> 7) & 1) == 1,
        ok96 && ((cmd96 as u32 >> 10) & 1) == 1
    );

    // BIOS 特性表 page7（电源模式支持位）
    let page7 = dchu.read_settings(7, 0, 256);
    let version = (u16::from(page7[0]) << 8) | u16::from(page7[1]);
    if version >= 256 {
        let mode_bits = page7[17];
        println!(
            "BIOS 特性表 v1: Quiet={} PowerSaving={} Performance={} Entertainment={}",
            mode_bits & 1 == 1,
            (mode_bits >> 1) & 1 == 1,
            (mode_bits >> 2) & 1 == 1,
            (mode_bits >> 3) & 1 == 1
        );
    } else {
        println!("BIOS 特性表 v0: 电源模式支持位 @offset17 = {}", page7[17]);
    }

    // WMI18 超频默认值包
    let p18 = dchu.read_package(18);
    let pl1_raw = u32_le(&p18, 8);
    let pl2_raw = u32_le(&p18, 12);
    let scale = |raw: u32| -> (u32, &'static str) {
        if raw > 1_000_000 {
            (raw / 1_000_000, "W(µW)")
        } else if raw > 1_000 {
            (raw / 1_000, "W(mW)")
        } else {
            (raw, "W")
        }
    };
    let (pl1, u1) = scale(pl1_raw);
    let (pl2, u2) = scale(pl2_raw);
    let uv_raw = u16_le(&p18, 176);
    let uv_default = if (uv_raw >> 15) & 1 == 1 {
        (uv_raw ^ 0x8000) as i32
    } else {
        -(uv_raw as i32)
    };
    let gt_raw = u16_le(&p18, 180);
    println!(
        "WMI18 默认: PL1={} {} PL2={} {} CPU电压偏移默认={}mV GT偏移默认={}(u16) PL时间窗={} VR={:#x}",
        pl1,
        u1,
        pl2,
        u2,
        uv_default,
        gt_raw,
        u16_le(&p18, 219),
        u32_le(&p18, 221)
    );

    // 实时读数冒烟（证明读通路）
    for (cmd, name) in [(29, "CPU温度"), (85, "GPU温度"), (16, "CPU频率MHz"), (101, "FanDuty")] {
        let (ok, v) = dchu.read_int(cmd);
        println!("read {name} (cmd {cmd}): ok={ok} value={v}");
    }

    println!();
    println!("== Phase 0.4 结论（平台级）==");
    if cpuoc || cpuoc_wmi122 {
        println!("CPUOC/undervolt: 平台声明支持（OEM CPU_OC 功能同款通道：AppSettings page6 + XTU）");
        println!("PL1/PL2: 可写（OEM 同款通道：page6 offset 33/37/41 + IsSaved）");
    } else {
        println!("CPUOC 能力位未置位：page6+XTU 通道不可靠，undervolt 结论待 MSR 0x150 直接测试");
    }
    println!("MSR 0x610/0x150 锁状态：需要 PawnIO 驱动，用管理员权限运行 `pl` / `uv-test` 子命令");
    Ok(())
}
