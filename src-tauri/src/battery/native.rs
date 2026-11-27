use super::{Io, model::*, protocol};
use crate::hal::Hal;
use crate::hal::oem_dchu::ComponentUnavailable;
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
    }
}
pub(super) fn system_power() -> Result<PowerStatus> {
    let mut raw = SYSTEM_POWER_STATUS::default();
    if unsafe { GetSystemPowerStatus(&mut raw) } == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(power(&raw))
}
/// 原厂组件自身的不可用与通道调用失败分开；其余传输错误都按通道无响应说明。
pub(super) fn unavailable(error: anyhow::Error) -> ChargeUnavailable {
    if error.downcast_ref::<ComponentUnavailable>().is_some() {
        ChargeUnavailable::Component(error)
    } else {
        ChargeUnavailable::Transport(error)
    }
}
impl Io for Native<'_> {
    fn charge(&self) -> Result<Option<ChargeState>, ChargeUnavailable> {
        let mut packet = [0; 256];
        packet[0] = 30;
        let response = self
            .hal
            .with_dchu(|io| io.exchange_package(4, &packet))
            .map_err(unavailable)?;
        protocol::charge(&response).map_err(ChargeUnavailable::Firmware)
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
