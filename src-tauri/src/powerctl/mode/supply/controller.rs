use super::{
    Review, Scope,
    control::{Report, Shared, source},
    record::FileStore,
    scheduler::Work,
    transaction::{self, Applied, Io, View},
};
use crate::hal::power_events::Source;
use anyhow::{Result, anyhow, bail};
use serde::Serialize;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex, MutexGuard},
    thread::JoinHandle,
};

mod operations;
#[cfg(test)]
mod tests;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Status {
    #[serde(flatten)]
    pub view: View,
    pub listener_online: bool,
    pub sleeping: bool,
    pub active: bool,
    pub can_resume: bool,
    pub issue: Option<String>,
    pub last: Option<Report>,
}
pub(crate) struct Controller {
    pub(super) store: FileStore,
    pub(super) profiles_path: PathBuf,
    pub(super) shared: Arc<Shared>,
    mutation: Mutex<()>,
    pub(super) worker: Mutex<Option<JoinHandle<()>>>,
}
impl Controller {
    pub fn new(path: PathBuf, profiles_path: PathBuf) -> Self {
        Self {
            store: FileStore(path),
            profiles_path,
            shared: Arc::new(Shared::default()),
            mutation: Mutex::new(()),
            worker: Mutex::new(None),
        }
    }
    pub(super) fn lock(&self) -> Result<MutexGuard<'_, ()>> {
        self.mutation
            .lock()
            .map_err(|_| anyhow!("供电模式操作状态异常"))
    }
    pub(super) fn read_in_control(&self, io: &impl Io) -> Result<Status> {
        self.status(io, Scope::All)
    }
    fn status(&self, io: &impl Io, scope: Scope) -> Result<Status> {
        let mut view = transaction::read_scoped(io, &self.store, scope)?;
        let ownership = crate::profiles::policy::ensure_inactive(&self.profiles_path);
        let target_issue = view
            .review
            .as_ref()
            .and_then(|review| review.unavailable_for(scope))
            .map(str::to_owned);
        let state = self.shared.lock()?;
        let ready =
            state.scheduler.online && !state.scheduler.sleeping && !state.scheduler.stopping;
        let matches = view
            .review
            .as_ref()
            .is_some_and(|review| state.scheduler.matches(source(review.on_ac)));
        view.can_enable &= ready && matches && !state.scheduler.enabled && ownership.is_ok();
        let active =
            view.enabled && ready && state.scheduler.enabled && matches && ownership.is_ok();
        view.can_restore &= ownership.is_ok();
        let can_resume = view.enabled
            && ready
            && matches
            && !state.scheduler.enabled
            && ownership.is_ok()
            && view.error.is_none()
            && target_issue.is_none();
        let issue = state
            .issue
            .clone()
            .or_else(|| ownership.err().map(|error| format!("{error:#}")))
            .or(target_issue);
        Ok(Status {
            can_resume,
            view,
            listener_online: state.scheduler.online && !state.scheduler.stopping,
            sleeping: state.scheduler.sleeping,
            active,
            issue,
            last: state.last.clone(),
        })
    }
    pub(super) fn enable_in_control(
        &self,
        io: &impl Io,
        expected: &str,
        now: u64,
        permit: &mut dyn FnMut() -> Result<()>,
    ) -> Result<()> {
        let epoch = self.shared.setup_ticket()?;
        let view = self.read_in_control(io)?;
        if view.view.revision != expected || !view.view.can_enable {
            bail!("供电模式预览已变化、自动档案冲突或通知未就绪");
        }
        let review = view
            .view
            .review
            .ok_or_else(|| anyhow!("供电模式事实不可用"))?;
        let mut check = || {
            self.shared.permit_setup(epoch)?;
            crate::profiles::policy::ensure_inactive(&self.profiles_path)?;
            permit()
        };
        transaction::enable(io, &self.store, expected, &mut check)?;
        let result = (|| {
            let after = io
                .review(Scope::All)?
                .ok_or_else(|| anyhow!("供电模式事实不可用"))?;
            if after.revision != review.revision {
                bail!("供电模式启用期间来源已变化");
            }
            check()?;
            self.shared.configure(epoch, source(after.on_ac), now)
        })();
        if let Err(error) = result {
            let recovery = (|| {
                let record = self
                    .store
                    .load()?
                    .ok_or_else(|| anyhow!("供电模式记录缺失"))?;
                if record.initial.is_some() || record.identity != review.identity {
                    bail!("供电模式记录已变化，保留原记录");
                }
                self.store.clear(&record)
            })();
            bail!(ccc_core::msr::with_recovery(
                format!("{error:#}"),
                recovery.map_err(|e| format!("{e:#}"))
            ));
        }
        Ok(())
    }
    pub(super) fn resume_in_control(
        &self,
        io: &impl Io,
        now: u64,
        permit: &mut dyn FnMut() -> Result<()>,
    ) -> Result<()> {
        if self.store.load()?.is_none_or(|record| !record.enabled) {
            return Ok(());
        }
        let epoch = self.shared.setup_ticket()?;
        let view = self.read_in_control(io)?;
        if let Some(error) = view.view.error {
            bail!("{error}");
        }
        crate::profiles::policy::ensure_inactive(&self.profiles_path)?;
        if !view.view.enabled {
            bail!("原厂供电模式保存许可已失效");
        }
        let review = view
            .view
            .review
            .ok_or_else(|| anyhow!("供电模式事实不可用"))?;
        if let Some(reason) = review.unavailable_reason() {
            bail!("原厂模式联动未通过预检：{reason}");
        }
        permit()?;
        self.shared.permit_setup(epoch)?;
        self.shared.configure(epoch, source(review.on_ac), now)
    }
    pub(super) fn preview_work(&self, io: &impl Io, work: Work) -> Result<(String, Review)> {
        self.shared.permit(work.epoch, work.source)?;
        let status = self.status(io, Scope::Event(work.trigger))?;
        if !status.active || status.view.error.is_some() || status.issue.is_some() {
            bail!(
                "{}",
                status
                    .view
                    .error
                    .or(status.issue)
                    .unwrap_or_else(|| "供电模式接管已暂停".into())
            );
        }
        let review = status
            .view
            .review
            .ok_or_else(|| anyhow!("供电模式事实不可用"))?;
        if source(review.on_ac) != work.source {
            bail!("供电模式实际来源与通知不一致");
        }
        Ok((status.view.revision, review))
    }
    pub(super) fn execute_in_control(
        &self,
        io: &impl Io,
        work: Work,
        expected: &str,
        permit: &mut dyn FnMut() -> Result<()>,
    ) -> Result<Option<Applied>> {
        let (revision, _) = self.preview_work(io, work)?;
        if revision != expected {
            bail!("供电模式系统授权期间完整预览已变化，未执行目标");
        }
        transaction::execute(io, &self.store, work.trigger, &mut || {
            self.shared.permit(work.epoch, work.source)?;
            crate::profiles::policy::ensure_inactive(&self.profiles_path)?;
            permit()
        })
    }
    pub fn pause(&self) -> Result<()> {
        self.shared.pause(None)
    }
    pub(super) fn next_work(
        &self,
        source: Source,
        now: u64,
        display_pending: bool,
    ) -> Result<Option<Work>> {
        let mut state = self.shared.lock()?;
        if !state.scheduler.enabled || !state.scheduler.online || state.scheduler.sleeping {
            return Ok(None);
        }
        state.scheduler.read_source(source, now)?;
        if display_pending {
            return Ok(None);
        }
        let work = state.scheduler.take(now);
        state.executing = work.is_some();
        Ok(work)
    }
    pub(crate) fn waiting_for_mode(&self, on_ac: Option<bool>) -> Result<bool> {
        let state = self.shared.lock()?;
        let scheduler = &state.scheduler;
        Ok(scheduler.enabled
            && (state.executing
                || scheduler.has_pending()
                || !scheduler.permits(scheduler.ticket(), source(on_ac))))
    }
}
