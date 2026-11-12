//! PawnIO 直连（T3 运行时 MSR：PL 0x610 / OC Mailbox 0x150）。
//!
//! 前提：PawnIO 驱动已安装（tools/probe/run-admin-probe.ps1 一次完成）。
//! IntelMSR 模块 blob 来自 msr-driver-rs（GPL-3，模块源 PawnIO.Modules/IntelMSR.p），
//! 提供 `ioctl_read_msr(msr)->u64` 与 `ioctl_write_msr(msr, value)`。
//!
//! 安全语义：本模块只做「读 + 受控写」；undervolt 值必须经 powerctl 钳位
//! （−150..+50 mV，安全设计 docs/design.md §4）。

use anyhow::{anyhow, bail, Result};
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows_sys::Win32::System::IO::DeviceIoControl;

const DEVICE_PATH: &[u16] = &[
    0x5C, 0x00, 0x5C, 0x00, 0x2E, 0x00, 0x5C, 0x00, 0x50, 0x00, 0x41, 0x00, 0x57, 0x00, 0x4E,
    0x00, 0x49, 0x00, 0x4F, 0x00, 0x00,
]; // "\\.\PawnIO"
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
                "PawnIO 驱动不可用：请以管理员运行 tools/probe/run-admin-probe.ps1 完成一次性安装"
            ));
        }
        let this = Self { handle };
        let blob = include_bytes!("../../assets/IntelMSR.bin");
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

/* ── OC Mailbox（0x150）编码（与 probe-cli/src/msr.rs 同源位布局）── */

pub const OC_MAILBOX: u32 = 0x150;
pub const MSR_PKG_POWER_LIMIT: u32 = 0x610;

pub fn mailbox_read(plane: u64) -> u64 {
    (1 << 63) | (plane << 40) | (1 << 36)
}

pub fn mailbox_write(plane: u64, offset_mv: i32) -> u64 {
    let x = ((offset_mv as f64) * 1.024).round() as i32 & 0xFFF;
    mailbox_read(plane) | (1 << 32) | (((x as u64) << 21) & 0xFFE0_0000)
}

/// 读取 plane 当前电压偏移（mV）。读协议本身经 mailbox 写命令。
pub fn read_voltage_offset(io: &PawnIo, plane: u64) -> Result<f64> {
    io.write_msr(OC_MAILBOX, mailbox_read(plane))?;
    let resp = io.read_msr(OC_MAILBOX)?;
    let plane_val = resp >> 40;
    let field = resp ^ (plane_val << 40);
    let y = ((field & 0xFFFF_FFFF) >> 21) as i32 & 0xFFF;
    let units = if y <= 1024 { y } else { -(2048 - y) };
    Ok(units as f64 / 1.024)
}

/// 设置电压偏移并回读校验，返回回读值。写入被忽略（锁）时回读 ≠ 目标。
pub fn set_voltage_offset(io: &PawnIo, plane: u64, mv: i32) -> Result<f64> {
    io.write_msr(OC_MAILBOX, mailbox_write(plane, mv))?;
    read_voltage_offset(io, plane)
}

/// 0x610 打包：PL1/PL2（1/8W 粒度）+ 各自时间窗保持原值不变（读改写）。
pub fn encode_power_limit(current: u64, pl1_w: Option<f64>, pl2_w: Option<f64>) -> u64 {
    let mut value = current;
    if let Some(pl1) = pl1_w {
        let units = (pl1 * 8.0).round() as u64;
        value = (value & !0x7FFF) | (units & 0x7FFF);
    }
    if let Some(pl2) = pl2_w {
        let units = (pl2 * 8.0).round() as u64;
        value = (value & !(0x7FFF_u64 << 32)) | ((units & 0x7FFF) << 32);
    }
    value
}
