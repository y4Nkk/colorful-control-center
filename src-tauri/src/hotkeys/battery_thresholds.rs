mod apply;
mod model;
mod native;
mod restore;
mod store;
mod transaction;

use crate::hal::Hal;
use anyhow::{Result, anyhow, bail};
pub(crate) use model::Profile;
use native::Native;
use std::{path::PathBuf, sync::Mutex};
use store::FileStore;

pub(super) struct Controller {
    store: FileStore,
    mutation: Mutex<()>,
}
impl Controller {
    pub fn new(path: PathBuf) -> Self {
        Self {
            store: FileStore(path),
            mutation: Mutex::new(()),
        }
    }
    pub fn read(&self, hal: &Hal) -> Result<Profile> {
        let _mutation = self
            .mutation
            .lock()
            .map_err(|_| anyhow!("电池阈值状态异常"))?;
        let _gate = hal.begin_write()?;
        Ok(transaction::preview(&Native(hal), &self.store))
    }
    pub fn run(
        &self,
        hal: &Hal,
        expected: &str,
        event: u8,
        mut permit: impl FnMut() -> Result<()>,
    ) -> Result<String> {
        let _mutation = self
            .mutation
            .lock()
            .map_err(|_| anyhow!("电池阈值状态异常"))?;
        let _gate = hal.begin_write()?;
        let io = Native(hal);
        match event {
            209 => transaction::apply(&io, &self.store, expected, &mut permit),
            210 => {
                permit()?;
                let profile = transaction::preview(&io, &self.store);
                if profile.identity.as_deref() != Some(expected) {
                    bail!("电池阈值授权范围已变化，请重新确认");
                }
                let Some(mut record) = self.store.load()? else {
                    return Ok("没有本应用的电池阈值记录，无需恢复".into());
                };
                restore::restore(&io, &self.store, &mut record, &mut permit)?;
                Ok("已恢复各计划的初始电池阈值".into())
            }
            _ => bail!("未知原厂电池阈值事件"),
        }
    }
    pub fn restore(&self, hal: &Hal, expected: &str) -> Result<()> {
        let _mutation = self
            .mutation
            .lock()
            .map_err(|_| anyhow!("电池阈值状态异常"))?;
        let _gate = hal.begin_write()?;
        let io = Native(hal);
        if transaction::preview(&io, &self.store)
            .restore_revision
            .as_deref()
            != Some(expected)
        {
            bail!("电池阈值初始恢复预览已变化，请重新查看");
        }
        let mut record = self
            .store
            .load()?
            .ok_or_else(|| anyhow!("没有待恢复的电池阈值记录"))?;
        restore::restore(&io, &self.store, &mut record, &mut || Ok(()))
    }
}

#[cfg(test)]
#[path = "battery_thresholds/native_tests.rs"]
mod native_tests;
#[cfg(test)]
#[path = "battery_thresholds/tests.rs"]
mod tests;
