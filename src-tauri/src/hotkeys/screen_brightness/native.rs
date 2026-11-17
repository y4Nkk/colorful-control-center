use super::{model::Snapshot, transaction::Io};
use crate::hal::{Hal, screen_brightness::Reading};
use anyhow::Result;
use ccc_core::display_brightness::Targets;

pub(super) struct Native<'a> {
    hal: &'a Hal,
    reading: Result<Reading>,
}
impl<'a> Native<'a> {
    pub fn new(hal: &'a Hal) -> Self {
        Self {
            hal,
            reading: hal.screen_brightness(),
        }
    }
}
impl Io for Native<'_> {
    fn snapshot(&self) -> Result<Snapshot> {
        let reading = self.panel()?;
        let plan = self.active()?;
        let levels = self.levels(plan)?;
        let targets = self.hal.with_dchu(|io| {
            Targets::from_packet(&io.read_package(17)?).map_err(anyhow::Error::msg)
        })?;
        let snapshot = Snapshot {
            reading,
            plan,
            levels,
            targets,
        };
        snapshot.validate()?;
        Ok(snapshot)
    }
    fn panel(&self) -> Result<Reading> {
        self.reading
            .as_ref()
            .cloned()
            .map_err(|error| anyhow::anyhow!("{error:#}"))
    }
    fn active(&self) -> Result<u128> {
        self.hal.system_power.active()
    }
    fn levels(&self, plan: u128) -> Result<[u32; 2]> {
        self.hal.system_power.brightness(plan)
    }
    fn write(&self, plan: u128, ac: bool, value: u32) -> Result<()> {
        self.hal.system_power.set_brightness(plan, ac, value)
    }
    fn activate(&self, plan: u128) -> Result<()> {
        self.hal.system_power.activate(plan)
    }
    fn ensure_writable(&self) -> Result<()> {
        self.hal.ensure_mode_change_allowed()
    }
}
