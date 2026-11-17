use super::*;
use crate::battery::{Io as BatteryIo, native::Native as BatteryNative};
pub struct Native<'a> {
    pub hal: &'a Hal,
}
impl Io for Native<'_> {
    fn capability(&self) -> Result<Option<Capability>> {
        self.hal.with_dchu(|d| {
            let page = d.read_bios_capabilities(0, 15)?;
            if !codec::supported(&page)? {
                return Ok(None);
            }
            codec::capability(&page, &d.read_package(17)?)
        })
    }
    fn charge(&self) -> Result<ChargeState> {
        BatteryNative { hal: self.hal }
            .charge()?
            .context("没有可读取的充电策略，不能建立节能恢复基线")
    }
    fn present(&self) -> Result<bool> {
        Ok(BatteryNative { hal: self.hal }.power()?.present == Some(true))
    }
    fn original(&self, cap: &Capability) -> Result<Option<Policy>> {
        original::read(std::path::Path::new(r"C:\ComboKey\EnergySave.ini"), cap)
    }
    fn clock(&self) -> Clock {
        let mut time = windows_sys::Win32::Foundation::SYSTEMTIME::default();
        unsafe {
            windows_sys::Win32::System::SystemInformation::GetLocalTime(&mut time);
        }
        Clock {
            weekday: time.wDayOfWeek as u8,
            hour: time.wHour as u8,
            minute: time.wMinute as u8,
            second: time.wSecond as u8,
        }
    }
    fn send(&self, command: u32) -> Result<()> {
        self.hal.with_dchu(|d| d.set_u32(118, command))
    }
    fn restore_charge(&self, charge: &ChargeState) -> Result<()> {
        self.charge()?.same_capabilities(charge)?;
        BatteryNative { hal: self.hal }.send(&charge.current)?;
        if self.charge()? != *charge {
            bail!("节能关联充电恢复回读不一致");
        }
        Ok(())
    }
}
