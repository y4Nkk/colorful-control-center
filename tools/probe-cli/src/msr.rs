//! MSR 访问：PawnIO 驱动（msr-driver-rs 只读 + IntelMSR 模块直写）。
//! IntelMSR.bin 来自 msr-driver-rs（GPL-3）随附的编译模块，模块提供
//! `ioctl_read_msr(msr) -> value` 与 `ioctl_write_msr(msr, value)`。

use anyhow::{anyhow, bail, Result};
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows_sys::Win32::System::IO::DeviceIoControl;

const DEVICE_PATH: &[u16] = &[0x5C, 0x00, 0x5C, 0x00, 0x2E, 0x00, 0x5C, 0x00, 0x50, 0x00, 0x41,
    0x00, 0x57, 0x00, 0x4E, 0x00, 0x49, 0x00, 0x4F, 0x00, 0x00]; // "\\.\PawnIO"
const IOCTL_PIO_LOAD_BINARY: u32 = (41394 << 16) | (0x821 << 2);
const IOCTL_PIO_EXECUTE_FN: u32 = (41394 << 16) | (0x841 << 2);
const GENERIC_READ: u32 = 0x8000_0000;
const GENERIC_WRITE: u32 = 0x4000_0000;
const FN_NAME_LEN: usize = 32;

pub struct PawnIo {
    handle: HANDLE,
}

unsafe impl Send for PawnIo {}
unsafe impl Sync for PawnIo {}

impl PawnIo {
    /// 打开 \\.\PawnIO 设备并加载 IntelMSR 模块。需要驱动已安装且以管理员运行。
    pub fn open_with_intel_msr() -> Result<Self> {
        let handle = unsafe {
            CreateFileW(
                DEVICE_PATH.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                std::ptr::null(),
                OPEN_EXISTING,
                0,
                std::ptr::null_mut(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            return Err(anyhow!(
                "无法打开 \\\\.\\PawnIO：驱动未安装（先运行 ccc-probe install，需管理员）或权限不足"
            ));
        }
        let this = Self { handle };
        let blob = include_bytes!("../assets/IntelMSR.bin");
        unsafe {
            let mut returned: u32 = 0;
            let ok = DeviceIoControl(
                handle,
                IOCTL_PIO_LOAD_BINARY,
                blob.as_ptr() as *const _,
                blob.len() as u32,
                std::ptr::null_mut(),
                0,
                &mut returned,
                std::ptr::null_mut(),
            );
            if ok == 0 {
                return Err(anyhow!(
                    "IntelMSR 模块加载失败（GetLastError={}）",
                    std::io::Error::last_os_error()
                ));
            }
        }
        Ok(this)
    }

    fn execute(&self, name: &str, input: &[u64], output_len: usize) -> Result<Vec<u64>> {
        assert!(name.len() < FN_NAME_LEN);
        let mut buffer = [0u8; FN_NAME_LEN];
        buffer[..name.len()].copy_from_slice(name.as_bytes());
        let mut in_buf = buffer.to_vec();
        for v in input {
            in_buf.extend_from_slice(&v.to_le_bytes());
        }
        let mut out_buf = vec![0u8; output_len * 8];
        let mut returned: u32 = 0;
        unsafe {
            let ok = DeviceIoControl(
                self.handle,
                IOCTL_PIO_EXECUTE_FN,
                in_buf.as_ptr() as *const _,
                in_buf.len() as u32,
                out_buf.as_mut_ptr() as *mut _,
                out_buf.len() as u32,
                &mut returned,
                std::ptr::null_mut(),
            );
            if ok == 0 {
                bail!(
                    "执行 {name} 失败（GetLastError={})",
                    std::io::Error::last_os_error()
                );
            }
        }
        Ok(out_buf
            .chunks_exact(8)
            .map(|c| u64::from_le_bytes(c.try_into().unwrap()))
            .collect())
    }

    pub fn read_msr(&self, msr: u32) -> Result<u64> {
        let out = self.execute("ioctl_read_msr", &[msr as u64], 1)?;
        Ok(out[0])
    }

    /// 写 MSR，返回写入后的旧值语义由模块决定（ioctl_write_msr 无返回 cell）。
    pub fn write_msr(&self, msr: u32, value: u64) -> Result<()> {
        self.execute("ioctl_write_msr", &[msr as u64, value], 0)?;
        Ok(())
    }
}

impl Drop for PawnIo {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.handle) };
    }
}

/* ── OC Mailbox（MSR 0x150）编码 ──
   协议来源：docs/oem-dchu-contract.md §10 与 intel-undervolt/undervolt.py 同款位布局。 */

pub const OC_MAILBOX: u32 = 0x150;
pub const PLANES: [(&str, u64); 5] = [
    ("cpu", 0),
    ("gpu", 1),
    ("cache", 2),
    ("uncore", 3),
    ("analogio", 4),
];

fn encode_offset(mv: i32) -> u64 {
    let x = ((mv as f64) * 1.024).round() as i32 & 0xFFF; // 12 位补码
    ((x as u64) << 21) & 0xFFE0_0000
}

fn decode_offset(field: u64) -> f64 {
    let y = ((field & 0xFFFF_FFFF) >> 21) as i32 & 0xFFF;
    let units = if y <= 1024 { y } else { -(2048 - y) };
    units as f64 / 1.024
}

fn mailbox_read(plane: u64) -> u64 {
    (1 << 63) | (plane << 40) | (1 << 36)
}

fn mailbox_write(plane: u64, offset_mv: i32) -> u64 {
    mailbox_read(plane) | (1 << 32) | encode_offset(offset_mv)
}

/// 读取指定 plane 的当前电压偏移（mV）。读协议本身需写 mailbox。
pub fn read_voltage_offset(io: &PawnIo, plane: u64) -> Result<f64> {
    io.write_msr(OC_MAILBOX, mailbox_read(plane))?;
    let resp = io.read_msr(OC_MAILBOX)?;
    let plane_val = resp >> 40;
    let field = resp ^ (plane_val << 40);
    Ok(decode_offset(field))
}

/// 设置电压偏移并回读校验。返回回读值。
pub fn set_voltage_offset(io: &PawnIo, plane: u64, mv: i32) -> Result<f64> {
    io.write_msr(OC_MAILBOX, mailbox_write(plane, mv))?;
    io.write_msr(OC_MAILBOX, mailbox_read(plane))?;
    let resp = io.read_msr(OC_MAILBOX)?;
    let plane_val = resp >> 40;
    let field = resp ^ (plane_val << 40);
    Ok(decode_offset(field))
}

/* ── 0x610 / 0xCE / 0xE2 解码 ── */

pub fn decode_pkg_power_limit(v: u64) -> String {
    let pl1 = (v & 0x7FFF) as f64 * 0.125;
    let pl2 = ((v >> 32) & 0x7FFF) as f64 * 0.125;
    let tw1 = (v >> 17) & 0x7F;
    let unit1 = (v >> 16) & 1;
    let w1 = (1.0 + tw1 as f64 / 128.0) * if unit1 == 1 { 1.0 } else { 0.1 };
    format!(
        "PL1={pl1:.3}W PL2={pl2:.3}W PL1时间窗={w1:.3}s | 锁位: PL1clamp={} PL2clamp={} LOCK(bit63)={}",
        (v >> 15) & 1,
        (v >> 47) & 1,
        (v >> 63) & 1
    )
}

pub fn decode_platform_info(v: u64) -> String {
    format!(
        "MaxNonTurboRatio={} | 可编程: Ratio={} TDP(PL)={} TCCOffset={}",
        (v >> 8) & 0xFF,
        (v >> 28) & 1,
        (v >> 29) & 1,
        (v >> 30) & 1
    )
}

pub fn decode_feature_config(v: u64) -> String {
    format!("raw={v:#x} | OC段使能(bit0)={} OC锁(bit1)={}", v & 1, (v >> 1) & 1)
}
