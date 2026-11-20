use super::{model::Rule, transaction::Io};
use crate::{
    battery::PowerStatus,
    hal::{Hal, screen_brightness::Reading},
};
use anyhow::{Result, bail};

pub struct Native<'a> {
    hal: &'a Hal,
    panel: Result<Reading>,
}
impl<'a> Native<'a> {
    pub fn new(hal: &'a Hal) -> Self {
        Self {
            hal,
            panel: hal.screen_brightness(),
        }
    }
}
impl Io for Native<'_> {
    fn known_firmware(&self) -> bool {
        self.hal.reviewed_dsdt
    }
    fn rule(&self) -> Result<Rule> {
        self.hal.with_dchu(|io| {
            let data = io.bios_capabilities()?;
            let customer = io.read_package(17)?[2];
            Ok(Rule {
                version: [data[0], data[1]],
                customer,
                feature: data[16] & 0x40 != 0,
            })
        })
    }
    fn panel(&self) -> Result<String> {
        self.panel
            .as_ref()
            .map(|value| value.panel.clone())
            .map_err(|error| anyhow::anyhow!("{error:#}"))
    }
    fn active(&self) -> Result<u128> {
        self.hal.system_power.active()
    }
    fn levels(&self, plan: u128) -> Result<[u32; 2]> {
        self.hal.system_power.brightness(plan)
    }
    fn write_level(&self, plan: u128, ac: bool, value: u32) -> Result<()> {
        self.hal.system_power.set_brightness(plan, ac, value)
    }
    fn cache(&self) -> Result<[u8; 2]> {
        self.hal.with_dchu(|io| {
            io.read_settings(1, 13, 2)?
                .try_into()
                .map_err(|_| anyhow::anyhow!("低电量亮度缓存长度无效"))
        })
    }
    fn write_cache(&self, index: usize, value: u8) -> Result<()> {
        if index >= 2 {
            bail!("低电量亮度缓存字段无效");
        }
        self.hal
            .with_dchu(|io| io.write_settings(1, 13 + index as i32, &[value]))
    }
    fn power(&self) -> Result<PowerStatus> {
        crate::battery::system_power()
    }
    fn signal(&self, value: u32) -> Result<()> {
        if !matches!(value, 2 | 3) {
            bail!("低电量原厂通知参数无效");
        }
        self.hal.with_dchu(|io| io.set_data_sub(121, 44, value))
    }
    fn activate(&self, plan: u128) -> Result<()> {
        self.hal.system_power.activate(plan)
    }
    fn ensure_writable(&self) -> Result<()> {
        self.hal.ensure_mode_change_allowed()?;
        #[cfg(test)]
        {
            bail!("隔离测试未注入原厂进程检查");
        }
        #[cfg(not(test))]
        {
            crate::hal::oem_owner::ensure_owner_absent()?;
            crate::hal::oem_owner::ensure_battery_owner_absent()
        }
    }
}
