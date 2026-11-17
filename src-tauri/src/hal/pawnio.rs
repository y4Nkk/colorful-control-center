//! PawnIO 直连（T3 运行时 MSR：PL 0x610 / OC Mailbox 0x150）。
//!
//! 前提：PawnIO 驱动已安装（tools/probe/run-admin-probe.ps1 一次完成）。
//! IntelMSR 模块 blob 来自 msr-driver-rs（GPL-3，模块源 PawnIO.Modules/IntelMSR.p），
//! 提供 `ioctl_read_msr(msr)->u64` 与 `ioctl_write_msr(msr, value)`。
//!
//! 安全语义：本模块只做「读 + 受控写」；undervolt 值必须经 powerctl 钳位
//! （−150..+50 mV，安全设计 docs/design.md §4）。

use anyhow::{Result, anyhow, bail};
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows_sys::Win32::System::IO::DeviceIoControl;

use ccc_core::msr::{MsrAccess, PAWNIO_DEVICE_PATH};
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
                PAWNIO_DEVICE_PATH.as_ptr(),
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
                "打开 \\\\.\\PawnIO 失败：{}（检查安装状态与权限）",
                std::io::Error::last_os_error()
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
        if returned as usize != out_buf.len() {
            bail!(
                "执行 {name} 返回长度异常：期望 {}，实际 {returned}",
                out_buf.len()
            );
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

impl MsrAccess for PawnIo {
    fn read(&self, address: u32) -> std::result::Result<u64, String> {
        self.read_msr(address).map_err(|e| format!("{e:#}"))
    }

    fn write(&self, address: u32, value: u64) -> std::result::Result<(), String> {
        self.write_msr(address, value).map_err(|e| format!("{e:#}"))
    }

    fn pause(&self) {
        std::thread::sleep(std::time::Duration::from_micros(100));
    }
}
