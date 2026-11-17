use super::{scheduler::Scheduler, transaction::Applied};
use crate::hal::power_events::{Event, Source};
use anyhow::{Result, anyhow, bail};
use ccc_core::oem_supply_mode::Trigger;
use serde::Serialize;
use std::sync::{Condvar, Mutex, MutexGuard};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Report {
    pub at_ms: u64,
    pub trigger: Trigger,
    pub applied: bool,
    pub previous_mode: Option<u8>,
    pub mode: Option<u8>,
    pub on_ac: Option<bool>,
    pub error: Option<String>,
}
#[derive(Default)]
pub(super) struct State {
    pub scheduler: Scheduler,
    pub executing: bool,
    startup_claimed: bool,
    pub issue: Option<String>,
    pub last: Option<Report>,
}
pub(super) struct Shared {
    pub state: Mutex<State>,
    pub wake: Condvar,
    #[cfg(all(not(test), not(control_sim)))]
    pub stopper: Mutex<Option<crate::hal::power_events::Stopper>>,
}
impl Default for Shared {
    fn default() -> Self {
        Self {
            state: Mutex::new(State::default()),
            wake: Condvar::new(),
            #[cfg(all(not(test), not(control_sim)))]
            stopper: Mutex::new(None),
        }
    }
}
impl Shared {
    pub fn claim_startup(&self) -> Result<Option<u64>> {
        let mut state = self.lock()?;
        if state.startup_claimed {
            return Ok(None);
        }
        state.startup_claimed = true;
        Ok(Some(state.scheduler.ticket()))
    }
    pub fn configure(&self, epoch: u64, source: Source, now: u64) -> Result<()> {
        let mut state = self.lock()?;
        if !state.scheduler.permits_setup(epoch) {
            bail!("供电模式预览期间事件或监听状态已变化");
        }
        state.scheduler.configure(source, Trigger::Startup, now)?;
        state.startup_claimed = true;
        state.issue = None;
        self.wake.notify_all();
        Ok(())
    }
    pub fn lock(&self) -> Result<MutexGuard<'_, State>> {
        self.state
            .lock()
            .map_err(|_| anyhow!("供电模式运行状态异常"))
    }
    pub fn setup_ticket(&self) -> Result<u64> {
        let state = self.lock()?;
        let scheduler = &state.scheduler;
        if !scheduler.online || scheduler.sleeping || scheduler.stopping {
            bail!("电源通知未就绪或系统正在休眠");
        }
        Ok(scheduler.ticket())
    }
    pub fn permit_setup(&self, epoch: u64) -> Result<()> {
        if !self.lock()?.scheduler.permits_setup(epoch) {
            bail!("供电模式预览期间事件或监听状态已变化");
        }
        Ok(())
    }
    pub fn permit(&self, epoch: u64, source: Source) -> Result<()> {
        if !self.lock()?.scheduler.permits(epoch, source) {
            bail!("供电模式事件已变化，执行权限已撤销");
        }
        Ok(())
    }
    pub fn permit_restore(&self, epoch: u64) -> Result<()> {
        let state = self.lock()?;
        if state.scheduler.sleeping || state.scheduler.stopping || state.scheduler.ticket() != epoch
        {
            bail!("供电模式恢复期间事件或停止状态已变化");
        }
        Ok(())
    }
    pub fn observe(&self, event: Event, now: u64) -> Result<()> {
        let mut state = self.lock()?;
        if !state.scheduler.stopping {
            state.scheduler.observe(event, now)?;
            self.wake.notify_all();
        }
        Ok(())
    }
    pub fn pause(&self, issue: Option<String>) -> Result<()> {
        let mut state = self.lock()?;
        state.scheduler.disable()?;
        state.startup_claimed = true;
        state.executing = false;
        state.issue = issue;
        self.wake.notify_all();
        Ok(())
    }
    pub fn failed(&self, error: String) {
        if let Ok(mut state) = self.lock() {
            let _ = state.scheduler.fail();
            state.executing = false;
            state.issue = Some(error);
        }
        self.wake.notify_all();
    }
    pub fn report(
        &self,
        trigger: Trigger,
        applied: Option<Applied>,
        error: Option<String>,
        now: u64,
    ) -> Result<()> {
        let mut state = self.lock()?;
        state.executing = false;
        if error.is_some() {
            state.scheduler.disable()?;
        }
        state.issue = error.clone();
        state.last = Some(Report {
            at_ms: now,
            trigger,
            applied: applied.is_some(),
            previous_mode: applied.map(|result| result.previous_mode),
            mode: applied.map(|result| result.mode),
            on_ac: applied.map(|result| result.on_ac),
            error,
        });
        self.wake.notify_all();
        Ok(())
    }
}
pub(super) fn source(on_ac: Option<bool>) -> Source {
    match on_ac {
        Some(true) => Source::Ac,
        Some(false) => Source::Battery,
        None => Source::Unknown,
    }
}
