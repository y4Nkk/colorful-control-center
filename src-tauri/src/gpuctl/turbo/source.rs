use crate::hal::Hal;
use anyhow::Result;
#[cfg(not(test))]
use anyhow::bail;
use ccc_core::gpu_turbo::{Capabilities, Limits, Offsets};
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    pub capabilities: Capabilities,
    pub limits: Limits,
    pub saved: [Offsets; 2],
    pub mode: u8,
    pub on_ac: Option<bool>,
    pub supply_error: Option<String>,
}

pub trait SourceIo {
    fn read(&self) -> Result<Source>;
}

pub struct Native<'a>(pub &'a Hal);
impl SourceIo for Native<'_> {
    #[cfg(not(test))]
    fn read(&self) -> Result<Source> {
        let (capabilities, limits, saved, mode) = self.0.with_dchu(|io| {
            let mut request = [0u8; 256];
            let packet = io.bios_capabilities()?;
            let wmi96 = if Capabilities::needs_wmi96(&packet).map_err(anyhow::Error::msg)? {
                let (valid, value) = io.read_int(96);
                valid.then_some(value)
            } else {
                None
            };
            let caps = Capabilities::from_packet(&packet, wmi96).map_err(anyhow::Error::msg)?;
            request[0] = 9;
            let limits = Limits::from_packet(&io.exchange_package(4, &request)?);
            let saved = Offsets::saved(&io.read_settings(5, 16, 8)?).map_err(anyhow::Error::msg)?;
            let mode = io.read_settings(1, 1, 1)?[0];
            if mode > 4 {
                bail!("当前 OEM 模式超出已核对范围");
            }
            Ok((caps, limits, saved, mode))
        })?;
        let (on_ac, supply_error) = match supply() {
            Ok(value) => (value, None),
            Err(error) => (None, Some(format!("{error:#}"))),
        };
        Ok(Source {
            capabilities,
            limits,
            saved,
            mode,
            on_ac,
            supply_error,
        })
    }
    #[cfg(test)]
    fn read(&self) -> Result<Source> {
        let _ = self.0;
        Err(anyhow::anyhow!(
            "隔离 GPU Turbo 未注入来源，禁止访问真实固件和系统"
        ))
    }
}

#[cfg(not(test))]
pub(crate) fn supply() -> Result<Option<bool>> {
    use windows_sys::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
    let mut status = SYSTEM_POWER_STATUS::default();
    if unsafe { GetSystemPowerStatus(&mut status) } == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(match status.ACLineStatus {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    })
}
