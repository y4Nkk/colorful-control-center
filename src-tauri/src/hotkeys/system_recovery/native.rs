use super::{Io, Result, Source};
use anyhow::{Context, bail};
use sha2::{Digest, Sha256};
use std::{ffi::OsString, io::Read, os::windows::ffi::OsStringExt, path::PathBuf};
use windows_sys::Win32::System::SystemInformation::GetSystemDirectoryW;

pub(super) struct Native;
impl Io for Native {
    fn source(&self) -> Result<Source> {
        let mut directory = [0u16; 32768];
        let length =
            unsafe { GetSystemDirectoryW(directory.as_mut_ptr(), directory.len() as u32) } as usize;
        if length == 0 || length >= directory.len() {
            bail!("无法确定Windows系统目录");
        }
        let path = PathBuf::from(OsString::from_wide(&directory[..length])).join("control.exe");
        let mut bytes = Vec::new();
        std::fs::File::open(&path)
            .context("无法读取Windows恢复入口")?
            .take(32 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)?;
        if bytes.is_empty() || bytes.len() > 32 * 1024 * 1024 {
            bail!("Windows控制面板程序长度无效");
        }
        Ok(Source {
            path,
            fingerprint: format!("{:x}", Sha256::digest(&bytes)),
        })
    }
    fn open(&self, source: &Source) -> Result<()> {
        // Windows恢复窗口由用户继续操作，不把它当作本软件可结束的子会话。
        std::process::Command::new(&source.path)
            .args(["/name", "Microsoft.Recovery"])
            .spawn()
            .context("打开Windows恢复入口失败")?;
        Ok(())
    }
}
