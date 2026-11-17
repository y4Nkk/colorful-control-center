use super::{Identity, Io, Observed};
use crate::hal::Hal;
use anyhow::{Result, bail};
use windows_sys::Win32::{
    Globalization::{CP_OEMCP, MultiByteToWideChar},
    System::SystemInformation::GetSystemDirectoryW,
};
#[path = "native/identity.rs"]
mod identity;
pub struct Native<'a> {
    pub hal: &'a Hal,
}
fn supported(caps: &[u8]) -> Result<bool> {
    if caps.len() < 19 {
        bail!("原厂显卡能力页长度不足");
    }
    if caps[..2] != [0, 0] && caps[..2] != [1, 0] {
        bail!("未知原厂显卡能力页版本");
    }
    Ok(caps[..2] == [1, 0] && caps[18] & 1 != 0)
}
impl Io for Native<'_> {
    fn observed(&self) -> Result<Option<Observed>> {
        self.hal.with_dchu(|dchu| {
            if !supported(&dchu.read_bios_capabilities(0, 19)?)? {
                return Ok(None);
            }
            let mut packet = [0; 256];
            packet[0] = 21;
            let raw = dchu.exchange_package(4, &packet)?;
            let observed = Observed {
                mode: raw[0],
                available_mask: raw[1],
            };
            observed.validate()?;
            Ok(Some(observed))
        })
    }
    fn identity(&self) -> Result<Identity> {
        identity::read()
    }
    fn send(&self, target: u8) -> Result<()> {
        if !self
            .observed()?
            .is_some_and(|observed| observed.supports(target))
        {
            bail!("固件未声明支持此显卡模式");
        }
        self.hal.with_dchu(|dchu| {
            let mut packet = [0; 256];
            packet[0] = 22;
            packet[1] = target;
            dchu.exchange_package(4, &packet)?;
            Ok(())
        })
    }
    fn reboot(&self) -> Result<()> {
        request_reboot()
    }
}
fn output_text(bytes: &[u8]) -> String {
    let size = unsafe {
        MultiByteToWideChar(
            CP_OEMCP,
            0,
            bytes.as_ptr(),
            bytes.len() as i32,
            std::ptr::null_mut(),
            0,
        )
    };
    if size <= 0 {
        return String::from_utf8_lossy(bytes).into_owned();
    }
    let mut text = vec![0; size as usize];
    unsafe {
        MultiByteToWideChar(
            CP_OEMCP,
            0,
            bytes.as_ptr(),
            bytes.len() as i32,
            text.as_mut_ptr(),
            size,
        );
    }
    String::from_utf16_lossy(&text)
}
fn request_reboot() -> Result<()> {
    use std::os::windows::process::CommandExt;
    let mut path = [0u16; 32768];
    let length = unsafe { GetSystemDirectoryW(path.as_mut_ptr(), path.len() as u32) } as usize;
    if length == 0 || length >= path.len() {
        bail!("无法定位Windows系统目录，未请求重启");
    }
    let executable =
        std::path::PathBuf::from(String::from_utf16_lossy(&path[..length])).join("shutdown.exe");
    let output = std::process::Command::new(executable)
        .args(["/r", "/t", "0"])
        .creation_flags(0x08000000)
        .output()?;
    if !output.status.success() {
        bail!(
            "Windows未接受重启请求（退出码{:?}）：{} {}",
            output.status.code(),
            output_text(&output.stdout),
            output_text(&output.stderr)
        );
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mode_capability_requires_its_own_extended_bit() {
        let mut caps = [0; 19];
        caps[0] = 1;
        assert!(!supported(&caps).unwrap());
        caps[18] = 1;
        assert!(supported(&caps).unwrap());
        caps[0] = 0;
        assert!(!supported(&caps).unwrap());
        caps[0] = 2;
        assert!(supported(&caps).is_err());
        assert!(supported(&caps[..18]).is_err());
    }
}
