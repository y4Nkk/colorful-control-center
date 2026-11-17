use super::{
    engine, facts,
    fields::{self, FileStore},
    native::Native,
    scheduler::Scheduler,
    state,
};
use crate::{
    display,
    hal::{Hal, power_events::Event},
};
use anyhow::{Result, anyhow, bail};
use ccc_core::oem_refresh::Trigger;
use serde::Serialize;
use std::{
    path::PathBuf,
    sync::{Arc, Condvar, Mutex, MutexGuard},
    thread::JoinHandle,
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Status {
    #[serde(flatten)]
    pub view: state::View,
    pub listener_online: bool,
    pub sleeping: bool,
    pub issue: Option<String>,
}
#[derive(Default)]
pub(super) struct RuntimeState {
    pub scheduler: Scheduler,
    pub trial: Option<(u64, u64)>,
    pub issue: Option<String>,
}
pub(super) struct Shared {
    pub state: Mutex<RuntimeState>,
    pub wake: Condvar,
    #[cfg(not(test))]
    pub stopper: Mutex<Option<crate::hal::power_events::Stopper>>,
}
impl Shared {
    pub fn lock(&self) -> Result<MutexGuard<'_, RuntimeState>> {
        self.state
            .lock()
            .map_err(|_| anyhow!("自动刷新率运行状态异常"))
    }
    pub fn available(&self) -> Result<()> {
        let state = self.lock()?;
        if !state.scheduler.online || state.scheduler.sleeping || state.scheduler.stopping {
            bail!("电源通知未就绪或系统正在休眠");
        }
        Ok(())
    }
    pub fn permit(&self, epoch: u64) -> Result<()> {
        if !self.lock()?.scheduler.permits(epoch) {
            bail!("自动刷新率条件已变化，执行权限已撤销");
        }
        Ok(())
    }
    pub fn permit_setup(&self, epoch: u64) -> Result<()> {
        if !self.lock()?.scheduler.permits_setup(epoch) {
            bail!("自动刷新率预览期间供电或监听状态已变化");
        }
        Ok(())
    }
    pub fn observe(&self, event: Event, now: u64) -> Result<()> {
        self.lock()?.scheduler.observe(event, now)?;
        self.wake.notify_all();
        Ok(())
    }
    pub fn fail(&self, error: String) {
        if let Ok(mut state) = self.lock() {
            let _ = state.scheduler.fail();
            state.issue = Some(error);
        }
        self.wake.notify_all();
    }
    pub fn issue(&self, error: Option<String>) {
        if let Ok(mut state) = self.lock() {
            state.issue = error;
        }
        self.wake.notify_all();
    }
}
pub(crate) struct Controller {
    pub(super) store: FileStore,
    pub(super) shared: Arc<Shared>,
    pub(super) worker: Mutex<Option<JoinHandle<()>>>,
}
impl Controller {
    pub(crate) fn mode_completed(
        &self,
        result: crate::powerctl::mode::supply::Applied,
        dc_mode_power: bool,
    ) -> Result<()> {
        if self.store.load()?.is_none_or(|record| !record.active()) {
            return Ok(());
        }
        self.shared.lock()?.scheduler.completed(
            result.mode,
            result.previous_mode,
            result.on_ac,
            result.trigger,
            dc_mode_power,
            display::now_ms(),
        )?;
        self.shared.wake.notify_all();
        Ok(())
    }
    pub fn new(path: PathBuf) -> Self {
        Self {
            store: FileStore(path),
            shared: Arc::new(Shared {
                state: Mutex::new(RuntimeState::default()),
                wake: Condvar::new(),
                #[cfg(not(test))]
                stopper: Mutex::new(None),
            }),
            worker: Mutex::new(None),
        }
    }
    pub fn read(&self, hal: &Hal, display: &display::Controller) -> Result<Status> {
        let _gate = hal.begin_write()?;
        let io = Native { hal, display };
        let mut view = state::read(&io, &self.store)?;
        let runtime = self.shared.lock()?;
        let ready =
            runtime.scheduler.online && !runtime.scheduler.stopping && !runtime.scheduler.sleeping;
        if let (Some((id, _)), Some(record), Some(facts)) =
            (runtime.trial, self.store.load()?, view.current.as_ref())
        {
            if display.automatic_trial_id()? == Some(id)
                && record.active()
                && !record.interrupted()
                && record.identity() == facts.identity
                && record.current() == facts.fields
                && record
                    .transition()
                    .is_some_and(|transition| facts.display.same_state(&transition.target))
            {
                view.error = None;
                view.active = true;
            }
        }
        let supply_matches = view.current.as_ref().map(|facts| {
            let source = match facts.on_ac {
                Some(true) => crate::hal::power_events::Source::Ac,
                Some(false) => crate::hal::power_events::Source::Battery,
                None => crate::hal::power_events::Source::Unknown,
            };
            runtime.scheduler.supply_matches(source)
        });
        view.can_start &= ready && !runtime.scheduler.enabled && supply_matches == Some(true);
        view.active &= ready && runtime.scheduler.enabled && runtime.scheduler.supply_known();
        let issue = runtime.issue.clone().or_else(|| {
            (ready && supply_matches == Some(false))
                .then(|| "供电通知与系统状态不一致，自动刷新率已暂停".into())
        });
        Ok(Status {
            view,
            listener_online: ready,
            sleeping: runtime.scheduler.sleeping,
            issue,
        })
    }
    pub fn enable(&self, hal: &Hal, display: &display::Controller, expected: &str) -> Result<()> {
        let _gate = hal.begin_write()?;
        self.shared.available()?;
        if display.trial_pending()? {
            bail!("先结束当前显示试用");
        }
        let io = Native { hal, display };
        let original = state::read(&io, &self.store)?
            .current
            .ok_or_else(|| anyhow!("自动刷新率事实不可用"))?;
        let epoch = self.shared.lock()?.scheduler.ticket();
        let mut permit = || {
            self.shared.permit_setup(epoch)?;
            permit_recovery(hal)
        };
        let mut recovery = || permit_recovery(hal);
        state::start(&io, &self.store, expected, &mut permit, &mut recovery)?;
        let result = (|| {
            let facts = facts::read(&io)?;
            if facts.identity != original.identity
                || facts.mode != original.mode
                || facts.on_ac != original.on_ac
                || !facts.display.same_state(&original.display)
            {
                bail!("自动刷新率预览期间主屏、模式或供电已变化");
            }
            self.shared.permit_setup(epoch)?;
            let on_ac = facts.on_ac.ok_or_else(|| anyhow!("供电状态未知"))?;
            self.shared.lock()?.scheduler.configure(
                facts.mode,
                on_ac,
                Trigger::UserChange,
                display::now_ms(),
            )
        })();
        if let Err(error) = result {
            let recovery = (|| {
                let mut record = self
                    .store
                    .load()?
                    .ok_or_else(|| anyhow!("自动刷新率恢复记录缺失"))?;
                fields::set_active(&self.store, &mut record, false)?;
                fields::restore(&io, &self.store, &mut record, &mut recovery)
            })();
            return Err(anyhow!(ccc_core::msr::with_recovery(
                format!("{error:#}"),
                recovery.map_err(|error| format!("{error:#}"))
            )));
        }
        self.shared.issue(None);
        Ok(())
    }
    pub fn restore(&self, hal: &Hal, display: &display::Controller, expected: &str) -> Result<()> {
        let _gate = hal.begin_write()?;
        let io = Native { hal, display };
        let view = state::read(&io, &self.store)?;
        if view.revision != expected || !view.can_restore {
            bail!("自动刷新率恢复预览已变化");
        }
        self.shared.lock()?.scheduler.disable()?;
        self.shared.wake.notify_all();
        let result = engine::stop_and_restore(&io, &self.store, display, expected, &mut || {
            permit_recovery(hal)
        });
        if result.is_ok() {
            let mut runtime = self.shared.lock()?;
            runtime.trial = None;
            runtime.issue = None;
        }
        result
    }
    pub fn keep(
        &self,
        hal: &Hal,
        display: &display::Controller,
        id: u64,
        expected: &display::Mode,
    ) -> Result<()> {
        let _gate = hal.begin_write()?;
        let epoch = self
            .shared
            .lock()?
            .trial
            .filter(|trial| trial.0 == id)
            .map(|trial| trial.1)
            .ok_or_else(|| anyhow!("自动刷新率试用已变化"))?;
        let io = Native { hal, display };
        let mut permit = || {
            self.shared.permit(epoch)?;
            permit_recovery(hal)
        };
        engine::keep(&io, &self.store, display, id, expected, &mut permit)?;
        engine::settle(&io, &self.store, display, id, &mut || permit_recovery(hal))?;
        self.shared.lock()?.trial = None;
        self.shared.wake.notify_all();
        Ok(())
    }
}
pub(super) fn permit_recovery(hal: &Hal) -> Result<()> {
    hal.ensure_mode_change_allowed()?;
    crate::hal::oem_owner::ensure_owner_absent()
}
