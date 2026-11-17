use super::transaction::Io;
use crate::hal::{Hal, power_plan::BatteryLevel};
use anyhow::{Result, bail};

pub struct Native<'a>(pub &'a Hal);
impl Io for Native<'_> {
    fn active(&self) -> Result<u128> {
        self.0.system_power.active()
    }
    fn schemes(&self) -> Result<Vec<u128>> {
        self.0.system_power.schemes()
    }
    fn levels(&self, plan: u128, level: BatteryLevel) -> Result<[u32; 2]> {
        self.0.system_power.battery_level(plan, level)
    }
    fn write(&self, plan: u128, level: BatteryLevel, ac: bool, value: u32) -> Result<()> {
        self.0
            .system_power
            .set_battery_level(plan, level, ac, value)
    }
    fn cache(&self) -> Result<[u8; 3]> {
        self.0.with_dchu(|io| {
            let data = io.read_settings(1, 47, 3)?;
            data.try_into()
                .map_err(|_| anyhow::anyhow!("原厂电池阈值缓存长度无效"))
        })
    }
    fn write_cache(&self, index: usize, value: u8) -> Result<()> {
        if index >= 3 {
            bail!("电池阈值缓存字段无效");
        }
        self.0
            .with_dchu(|io| io.write_settings(1, 47 + index as i32, &[value]))?;
        if self.cache()?[index] != value {
            bail!("原厂电池阈值缓存回读不一致");
        }
        Ok(())
    }
    fn activate(&self, plan: u128) -> Result<()> {
        self.0.system_power.activate(plan)
    }
    fn ensure_writable(&self) -> Result<()> {
        self.0.ensure_mode_change_allowed()
    }
}
