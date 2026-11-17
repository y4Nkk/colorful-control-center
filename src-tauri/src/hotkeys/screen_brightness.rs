mod low_battery;
mod model;
mod native;
mod transaction;
use crate::{hal::Hal, osd};
use anyhow::{Result, anyhow, bail};
pub(crate) use low_battery::Profile as LowBatteryProfile;
use model::Job;
pub(crate) use model::Profile;
use native::Native;
use std::cell::Cell;
use std::{
    path::PathBuf,
    sync::{Mutex, MutexGuard},
};
use transaction::{Io, State};

#[derive(Default)]
pub(super) struct Controller {
    state: Mutex<State>,
    low_store: Option<low_battery::FileStore>,
}
impl Controller {
    pub fn new(path: PathBuf) -> Self {
        Self {
            state: Mutex::new(State::default()),
            low_store: Some(low_battery::FileStore(path)),
        }
    }
    fn scope(&self) -> Result<MutexGuard<'_, State>> {
        self.state
            .lock()
            .map_err(|_| anyhow!("屏幕亮度恢复状态异常"))
    }
    fn low_store(&self) -> Result<&low_battery::FileStore> {
        self.low_store
            .as_ref()
            .ok_or_else(|| anyhow!("低电量亮度恢复记录未配置"))
    }
    fn ensure_low_clear(&self) -> Result<()> {
        if self
            .low_store
            .as_ref()
            .map(|store| store.load())
            .transpose()?
            .flatten()
            .is_some()
        {
            bail!("低电量亮度仍待恢复，请先恢复该事件接管的字段");
        }
        Ok(())
    }
    pub fn read_alert(&self, hal: &Hal) -> Result<LowBatteryProfile> {
        let io = low_battery::Native::new(hal);
        let _state = self.scope()?;
        let _gate = hal.begin_write()?;
        Ok(low_battery::preview_alert(&io, self.low_store()?))
    }
    pub fn alert_absolute(
        &self,
        hal: &Hal,
        expected: &str,
        value: u8,
        mut permit: impl FnMut() -> Result<()>,
    ) -> Result<()> {
        let io = low_battery::Native::new(hal);
        let _state = self.scope()?;
        let _gate = hal.begin_write()?;
        low_battery::absolute(&io, self.low_store()?, expected, value, &mut permit)
    }
    pub fn alert_low(
        &self,
        hal: &Hal,
        expected: &str,
        mut permit: impl FnMut() -> Result<()>,
    ) -> Result<()> {
        let io = low_battery::Native::new(hal);
        let _state = self.scope()?;
        let _gate = hal.begin_write()?;
        low_battery::alert_low(&io, self.low_store()?, expected, &mut permit)
    }
    pub fn read_low(&self, hal: &Hal) -> Result<low_battery::Profile> {
        let io = low_battery::Native::new(hal);
        let _state = self.scope()?;
        let _gate = hal.begin_write()?;
        Ok(low_battery::preview(&io, self.low_store()?))
    }
    pub fn run_low(
        &self,
        hal: &Hal,
        expected: &str,
        event: u8,
        mut permit: impl FnMut() -> Result<()>,
    ) -> Result<String> {
        let io = low_battery::Native::new(hal);
        let _state = self.scope()?;
        let _gate = hal.begin_write()?;
        low_battery::event(&io, self.low_store()?, expected, event, &mut permit)
    }
    pub fn check_low(
        &self,
        hal: &Hal,
        expected: &str,
        mut permit: impl FnMut() -> Result<()>,
    ) -> Result<Option<String>> {
        let io = low_battery::Native::new(hal);
        let _state = self.scope()?;
        let _gate = hal.begin_write()?;
        low_battery::check(&io, self.low_store()?, expected, &mut permit)
    }
    pub fn restore_low(&self, hal: &Hal, expected: &str) -> Result<()> {
        let io = low_battery::Native::new(hal);
        let _state = self.scope()?;
        let _gate = hal.begin_write()?;
        low_battery::restore(&io, self.low_store()?, expected)
    }
    pub fn read(&self, hal: &Hal) -> Result<Profile> {
        let io = Native::new(hal);
        let state = self.scope()?;
        let _gate = hal.begin_write()?;
        let mut profile = state.preview(&io);
        if !profile.initial.is_empty() {
            if let Err(error) = self.ensure_low_clear() {
                profile.restore_revision = None;
                profile.restore_error = Some(format!("{error:#}"));
            }
        }
        Ok(profile)
    }
    fn job(&self, hal: &Hal, expected: &str, event: u8) -> Result<Job> {
        let io = Native::new(hal);
        let state = self.scope()?;
        let _gate = hal.begin_write()?;
        self.ensure_low_clear()?;
        let snapshot = io.snapshot()?;
        state.compatible(&snapshot.reading.panel)?;
        Job::new(&snapshot, expected, event)
    }
    pub fn run(
        &self,
        hal: &Hal,
        expected: &str,
        event: u8,
        mut permit: impl FnMut() -> Result<bool>,
        mut wait: impl FnMut() -> Result<bool>,
    ) -> Result<Option<osd::Message>> {
        let cancelled = Cell::new(false);
        let mut check = || {
            if !permit()? {
                cancelled.set(true);
                bail!("屏幕亮度策略已取消");
            }
            Ok(())
        };
        if let Err(error) = check() {
            if cancelled.get() {
                return Ok(None);
            }
            return Err(error);
        }
        let mut job = self.job(hal, expected, event)?;
        let result = (|| {
            check()?;
            if !job.needed {
                return Ok(());
            }
            for attempt in 0..=3 {
                if !wait()? {
                    cancelled.set(true);
                    bail!("屏幕亮度策略已取消");
                }
                check()?;
                let io = Native::new(hal);
                check()?;
                let mut state = self.scope()?;
                let _gate = hal.begin_write()?;
                self.ensure_low_clear()?;
                if state.step(&io, &mut job, attempt < 3, &mut check)? {
                    check()?;
                    return Ok(());
                }
            }
            bail!("屏幕亮度策略未完成")
        })();
        if let Err(error) = result {
            return self.recover(hal, &job, error, cancelled.get());
        }
        Ok(Some(osd::Message::result(
            "屏幕亮度策略",
            if job.before.is_some() {
                format!(
                    "已按原厂 {}% 目标更新计划；实际读数已核对或供电已转插电",
                    job.target
                )
            } else {
                "当前无需降低屏幕亮度，未修改计划".into()
            },
        )))
    }
    pub fn restore(&self, hal: &Hal, expected: &str) -> Result<()> {
        let io = Native::new(hal);
        let state = self.scope()?;
        let _gate = hal.begin_write()?;
        self.ensure_low_clear()?;
        state.restore(&io, expected)
    }
    fn recover(
        &self,
        hal: &Hal,
        job: &Job,
        error: anyhow::Error,
        cancelled: bool,
    ) -> Result<Option<osd::Message>> {
        if job.before.is_none() {
            return if cancelled {
                Ok(None)
            } else {
                Err(error.context("屏幕亮度策略未完成，未执行计划写入"))
            };
        }
        let io = Native::new(hal);
        let state = self.scope()?;
        let _gate = hal.begin_write()?;
        match state.rollback(&io, job) {
            Ok(()) if cancelled => Ok(None),
            Ok(()) => Err(error.context("屏幕亮度策略未完成，本次已尝试字段已恢复")),
            Err(restore) => Err(anyhow!(
                "屏幕亮度策略失败：{error:#}；本次恢复失败：{restore:#}"
            )),
        }
    }
}

#[cfg(test)]
mod tests;
