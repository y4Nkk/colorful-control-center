use super::{
    events::Scheduler,
    model::{Binding, Record, Trigger},
    storage::Store,
};
use crate::{
    hal::power_events::{Event, Route},
    profiles::application::Permission,
};
use anyhow::{Result, anyhow, bail};
use std::{
    path::PathBuf,
    sync::{Arc, Condvar, Mutex, MutexGuard},
    time::Duration,
};

pub(super) struct EventState {
    pub scheduler: Scheduler,
    pub paused: bool,
    pub online: bool,
    pub stopping: bool,
    pub route: Option<Route>,
    pub listener_error: Option<String>,
    pub last_issue: Option<String>,
}
pub(super) struct Shared {
    pub path: PathBuf,
    pub config: Mutex<std::result::Result<Store, String>>,
    pub events: Mutex<EventState>,
    pub wake: Condvar,
    #[cfg(not(test))]
    pub stopper: Mutex<Option<crate::hal::power_events::Stopper>>,
}
pub(super) struct Work {
    pub trigger: Trigger,
    pub epoch: u64,
    pub record: Record,
    pub binding: Binding,
}

impl Shared {
    pub fn lock_config(&self) -> Result<MutexGuard<'_, std::result::Result<Store, String>>> {
        self.config
            .lock()
            .map_err(|_| anyhow!("自动档案配置状态异常"))
    }
    pub fn lock_events(&self) -> Result<MutexGuard<'_, EventState>> {
        self.events
            .lock()
            .map_err(|_| anyhow!("自动档案事件状态异常"))
    }
    pub fn record(&self) -> Result<Record> {
        let result = (|| {
            let config = self.lock_config()?;
            let store = config.as_ref().map_err(|error| anyhow!(error.clone()))?;
            store.check()?;
            Ok(store.record())
        })();
        if let Err(error) = &result {
            let mut state = self.lock_events()?;
            state.paused = true;
            let rules = state.scheduler.rules;
            state.scheduler.configure(false, rules)?;
            drop(state);
            self.issue(Some(format!("{error:#}")));
        }
        result
    }
    pub fn observe(&self, event: Event, now: u64) -> Result<()> {
        let mut state = self.lock_events()?;
        if !state.stopping {
            state.scheduler.observe(event, now)?;
            self.wake.notify_all();
        }
        Ok(())
    }
    pub fn ready(&self, route: Route) -> Result<()> {
        let config = self.lock_config()?;
        let record = config
            .as_ref()
            .map_err(|error| anyhow!(error.clone()))
            .and_then(|store| {
                store.check()?;
                Ok(store.record())
            });
        let mut state = self.lock_events()?;
        if state.stopping {
            return Ok(());
        }
        if let Some(error) = &state.listener_error {
            bail!("{error}");
        }
        state.online = true;
        state.route = Some(route);
        let record = match record {
            Ok(record) => record,
            Err(error) => {
                state.paused = true;
                state.last_issue = Some(format!("{error:#}"));
                Record::default()
            }
        };
        let enabled = record.enabled && !state.paused;
        state.scheduler.configure(enabled, record.rules())?;
        self.wake.notify_all();
        Ok(())
    }
    pub fn failed(&self, error: String) {
        if let Ok(mut state) = self.lock_events() {
            state.online = false;
            state.listener_error = Some(error);
            let rules = state.scheduler.rules;
            let _ = state.scheduler.configure(false, rules);
            self.wake.notify_all();
        }
    }
    pub fn issue(&self, issue: Option<String>) {
        let changed = if let Ok(mut state) = self.lock_events() {
            let changed = state.last_issue != issue;
            state.last_issue = issue.clone();
            changed
        } else {
            false
        };
        if changed {
            if let Some(issue) = issue {
                log::warn!("[power-policy] {issue}");
            }
        }
    }
    pub fn check(&self, epoch: u64, expected: &Record) -> Result<()> {
        {
            let state = self.lock_events()?;
            if !state.online || state.stopping || !state.scheduler.permits(epoch) {
                bail!("自动档案事件或执行授权已失效");
            }
        }
        if self.record()? != *expected {
            bail!("自动档案策略已变化，请重新确认");
        }
        let state = self.lock_events()?;
        if !state.online || state.stopping || !state.scheduler.permits(epoch) {
            bail!("自动档案事件或执行授权已失效");
        }
        Ok(())
    }
    pub fn permission(
        self: &Arc<Self>,
        epoch: u64,
        expected: Record,
        trigger: Trigger,
    ) -> Permission {
        let expected = Arc::new(Mutex::new(expected));
        let checked = expected.clone();
        let shared = self.clone();
        let bound = self.clone();
        Permission::with_mode_driver(
            move || {
                let snapshot = checked
                    .lock()
                    .map_err(|_| anyhow!("自动档案许可状态异常"))?
                    .clone();
                shared.check(epoch, &snapshot)
            },
            move |facts| {
                let mut expected = expected
                    .lock()
                    .map_err(|_| anyhow!("自动档案许可状态异常"))?;
                bound.check(epoch, &expected)?;
                let mut config = bound.lock_config()?;
                let store = config.as_mut().map_err(|error| anyhow!(error.clone()))?;
                {
                    let state = bound.lock_events()?;
                    if !state.online || state.stopping || !state.scheduler.permits(epoch) {
                        bail!("自动档案执行授权已失效，未记录GPU驱动");
                    }
                }
                *expected = store.bind_mode_driver(&expected, trigger, facts)?;
                Ok(())
            },
            crate::hal::authorization::require_elevated,
        )
    }
    pub fn take(&self, now: u64) -> Result<Option<Work>> {
        let record = self.record()?;
        let mut state = self.lock_events()?;
        if !state.online || state.stopping {
            return Ok(None);
        }
        Ok(state.scheduler.take(now).and_then(|p| {
            record.binding(p.trigger).cloned().map(|binding| Work {
                trigger: p.trigger,
                epoch: p.epoch,
                record,
                binding,
            })
        }))
    }
    pub fn wait(&self) -> Result<bool> {
        let state = self.lock_events()?;
        if state.stopping {
            return Ok(false);
        }
        let (state, _) = self
            .wake
            .wait_timeout(state, Duration::from_millis(200))
            .map_err(|_| anyhow!("自动档案事件等待异常"))?;
        Ok(!state.stopping)
    }
}
