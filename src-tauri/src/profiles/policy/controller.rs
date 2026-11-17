use super::{
    events::Scheduler,
    model::{Binding, Request, Selection, View},
    runtime::{EventState, Shared},
    storage::Store,
};
use crate::{
    hal::Hal,
    profiles::{self, policy::Record},
};
use anyhow::{Result, anyhow, bail};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Condvar, Mutex},
    thread::JoinHandle,
};

pub(crate) struct Controller {
    pub(super) shared: Arc<Shared>,
    pub(super) worker: Mutex<Option<JoinHandle<()>>>,
}
impl Controller {
    #[cfg(test)]
    pub(crate) fn test_ready(&self) -> Result<()> {
        self.shared.ready(crate::hal::power_events::Route {
            modern_standby: Some(false),
            display_events: false,
            warnings: Vec::new(),
        })
    }
    #[cfg(test)]
    pub(crate) fn test_ac_permission(
        &self,
        now: u64,
    ) -> Result<crate::profiles::application::Permission> {
        use crate::hal::power_events::{Event, Source};
        self.shared.observe(Event::Source(Source::Battery), now)?;
        self.shared.observe(Event::Source(Source::Ac), now + 1)?;
        let work = self
            .shared
            .take(now + 2000)?
            .ok_or_else(|| anyhow!("隔离自动档案没有到期的交流规则"))?;
        Ok(self
            .shared
            .permission(work.epoch, work.record, work.trigger))
    }
    pub fn new(path: PathBuf) -> Self {
        let config = Store::open(path.clone()).map_err(|error| format!("{error:#}"));
        if let Err(error) = &config {
            log::warn!("[power-policy] {error}");
        }
        Self {
            shared: Arc::new(Shared {
                path,
                config: Mutex::new(config),
                events: Mutex::new(EventState {
                    scheduler: Scheduler::default(),
                    paused: false,
                    online: false,
                    stopping: false,
                    route: None,
                    listener_error: None,
                    last_issue: None,
                }),
                wake: Condvar::new(),
                #[cfg(not(test))]
                stopper: Mutex::new(None),
            }),
            worker: Mutex::new(None),
        }
    }

    pub fn read(&self) -> Result<View> {
        let configuration_error = self.shared.record().err().map(|error| format!("{error:#}"));
        let reset_revision = if configuration_error.is_some() {
            super::storage::invalid_revision(&self.shared.path)?
        } else {
            None
        };
        let config = self.shared.lock_config()?;
        let store = config.as_ref().ok();
        let state = self.shared.lock_events()?;
        Ok(View {
            record: store.map(Store::record),
            initial: store.and_then(Store::initial_record),
            paused: state.paused,
            active: state.online && !state.stopping && state.scheduler.enabled,
            source: state.scheduler.source,
            sleeping: state.scheduler.sleeping(),
            pending: state.scheduler.pending.map(|p| p.trigger),
            route: state.route.clone(),
            configuration_error,
            reset_revision,
            listener_error: state.listener_error.clone(),
            last_issue: state.last_issue.clone(),
        })
    }

    pub fn save(
        &self,
        context: &crate::powerctl::mode::links::Context<'_>,
        dir: &Path,
        request: &Request,
    ) -> Result<()> {
        let hal = context.hal;
        if self.shared.record()? != request.expected {
            bail!("自动档案预览已变化，请重新确认");
        }
        let target = Record {
            version: 2,
            enabled: request.enabled,
            ac: review(context, dir, request.ac.as_ref())?,
            battery: review(context, dir, request.battery.as_ref())?,
            resume: review(context, dir, request.resume.as_ref())?,
        };
        target.validate()?;
        self.commit(hal, &request.expected, Some(&target))
    }

    fn commit(&self, hal: &Hal, expected: &Record, target: Option<&Record>) -> Result<()> {
        let _gate = hal.begin_write()?;
        let mut config = self.shared.lock_config()?;
        let store = config.as_mut().map_err(|error| anyhow!(error.clone()))?;
        store.check()?;
        if store.record() != *expected {
            bail!("自动档案预览已变化，请重新确认");
        }
        if target
            .cloned()
            .or_else(|| store.initial_record())
            .is_some_and(|record| record.enabled)
        {
            hal.ensure_automatic_profile_allowed()?;
        }
        {
            let mut state = self.shared.lock_events()?;
            if state.stopping {
                bail!("应用正在退出，未修改自动档案策略");
            }
            if target.is_some_and(|target| target.enabled) && !state.online {
                bail!("电源通知尚未就绪，不能启用自动档案");
            }
            state.scheduler.configure(false, store.record().rules())?;
        }
        let result = match target {
            Some(target) => store.save(expected, target),
            None => store.restore(expected),
        };
        let current = store.record();
        let mut state = self.shared.lock_events()?;
        if result.is_ok() {
            state.paused = target.is_none();
            state.last_issue = None;
        }
        let enabled = current.enabled && !state.paused && state.online && !state.stopping;
        state.scheduler.configure(enabled, current.rules())?;
        self.shared.wake.notify_all();
        result
    }

    pub fn pause(&self) -> Result<()> {
        let mut state = self.shared.lock_events()?;
        state.paused = true;
        let rules = state.scheduler.rules;
        state.scheduler.configure(false, rules)?;
        self.shared.wake.notify_all();
        Ok(())
    }

    pub fn restore(&self, hal: &Hal, expected: &Record) -> Result<()> {
        self.commit(hal, expected, None)
    }
    pub fn reset(&self, hal: &Hal, expected_revision: &str) -> Result<()> {
        self.pause()?;
        let _gate = hal.begin_write()?;
        let mut config = self.shared.lock_config()?;
        if self.shared.lock_events()?.stopping {
            bail!("应用正在退出，未清除规则");
        }
        *config = Ok(super::storage::reset(&self.shared.path, expected_revision)?);
        let mut state = self.shared.lock_events()?;
        state.scheduler.configure(false, [false; 3])?;
        state.last_issue = None;
        self.shared.wake.notify_all();
        Ok(())
    }

    pub fn reload(&self) -> Result<()> {
        self.pause()?;
        let mut config = self.shared.lock_config()?;
        match config.as_mut() {
            Ok(store) => store.reload()?,
            Err(_) => *config = Ok(Store::open(self.shared.path.clone())?),
        }
        self.shared.lock_events()?.last_issue = None;
        Ok(())
    }

    pub fn stop(&self) -> Result<()> {
        {
            let mut state = self.shared.lock_events()?;
            state.stopping = true;
            state.online = false;
            let rules = state.scheduler.rules;
            state.scheduler.configure(false, rules)?;
            self.shared.wake.notify_all();
        }
        #[cfg(not(test))]
        if let Some(stopper) = self
            .shared
            .stopper
            .lock()
            .map_err(|_| anyhow!("电源通知停止状态异常"))?
            .as_ref()
        {
            stopper.stop();
        }
        let mut worker = self
            .worker
            .lock()
            .map_err(|_| anyhow!("自动档案工作线程状态异常"))?;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while worker.as_ref().is_some_and(|worker| !worker.is_finished())
            && std::time::Instant::now() < deadline
        {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        if worker.as_ref().is_some_and(|worker| !worker.is_finished()) {
            bail!("自动档案仍在结束授权或恢复，执行权限已撤销");
        }
        if let Some(worker) = worker.take() {
            worker
                .join()
                .map_err(|_| anyhow!("自动档案工作线程退出异常"))?;
        }
        Ok(())
    }
}

fn review(
    context: &crate::powerctl::mode::links::Context<'_>,
    dir: &Path,
    selection: Option<&Selection>,
) -> Result<Option<Binding>> {
    let Some(selection) = selection else {
        return Ok(None);
    };
    if profiles::load_from(dir, &selection.file.name)? != selection.file {
        bail!("自动档案内容已变化，请重新预览");
    }
    let _gate = context.hal.begin_write()?;
    if profiles::preview_authorization(context, &selection.file.settings)? != selection.mode {
        bail!("自动档案模式联动已变化，请重新预览");
    }
    Ok(Some(Binding::reviewed(
        &selection.file,
        selection
            .mode
            .as_ref()
            .map(|mode| mode.grant())
            .transpose()?,
    )?))
}
