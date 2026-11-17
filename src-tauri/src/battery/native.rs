use super::{Io, model::*, protocol};
use crate::hal::Hal;
use anyhow::Result;
use windows_sys::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};

pub struct Native<'a> {
    pub hal: &'a Hal,
}
pub fn power(raw: &SYSTEM_POWER_STATUS) -> PowerStatus {
    let present = (raw.BatteryFlag != 255).then_some(raw.BatteryFlag & 128 == 0);
    let ac_online = match raw.ACLineStatus {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    };
    PowerStatus {
        present,
        ac_online,
        charging: present.filter(|v| *v).map(|_| raw.BatteryFlag & 8 != 0),
        percent: (present == Some(true) && raw.BatteryLifePercent <= 100)
            .then_some(raw.BatteryLifePercent),
        remaining_seconds: (present == Some(true)
            && ac_online == Some(false)
            && raw.BatteryLifeTime != u32::MAX)
            .then_some(raw.BatteryLifeTime),
        windows_battery_saver: match raw.SystemStatusFlag {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        },
    }
}
pub(super) fn system_power() -> Result<PowerStatus> {
    let mut raw = SYSTEM_POWER_STATUS::default();
    if unsafe { GetSystemPowerStatus(&mut raw) } == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(power(&raw))
}
impl Io for Native<'_> {
    fn charge(&self) -> Result<Option<ChargeState>> {
        self.hal.with_dchu(|io| {
            let mut packet = [0; 256];
            packet[0] = 30;
            protocol::charge(&io.exchange_package(4, &packet)?)
        })
    }
    fn send(&self, values: &ChargeValues) -> Result<()> {
        self.hal.with_dchu(|io| {
            io.exchange_package(4, &protocol::request(values))?;
            Ok(())
        })
    }
    fn power(&self) -> Result<PowerStatus> {
        system_power()
    }
    fn diagnostics(&self) -> Result<Option<Diagnostics>> {
        self.hal.with_dchu(|io| {
            if !protocol::utility_supported(&io.read_bios_capabilities(0, 15)?)? {
                return Ok(None);
            }
            Ok(Some(protocol::diagnostics(&io.read_package(7)?)))
        })
    }
}
