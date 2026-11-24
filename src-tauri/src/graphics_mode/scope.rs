use super::*;
use crate::hal::Hal;
use std::sync::MutexGuard;

pub(crate) struct Scope<'a> {
    controller: &'a Controller,
    _guard: MutexGuard<'a, ()>,
    active: Option<transaction::Plan>,
}
impl Controller {
    pub(crate) fn scope(&self) -> Result<Scope<'_>> {
        Ok(Scope {
            controller: self,
            _guard: self
                .gate
                .lock()
                .map_err(|_| anyhow!("显卡选择记录锁异常"))?,
            active: None,
        })
    }
}
impl Scope<'_> {
    pub(crate) fn profile_dirty(&self) -> bool {
        self.active.as_ref().is_some_and(transaction::Plan::dirty)
    }

    pub(super) fn with_io<T>(&self, hal: &Hal, f: impl FnOnce(&dyn Io) -> Result<T>) -> Result<T> {
        #[cfg(test)]
        {
            let _ = hal;
            f(self
                .controller
                .io
                .as_deref()
                .context("隔离显卡选择缺少显式IO，禁止使用原生提供者")?)
        }
        #[cfg(not(test))]
        f(&native::Native { hal })
    }
    pub(crate) fn read(&self, hal: &Hal) -> Result<Profile> {
        self.with_io(hal, |io| super::profile(io, &self.controller.store))
    }
    pub(crate) fn capture(&self, hal: &Hal) -> Result<Option<profile::Target>> {
        self.with_io(hal, |io| {
            preview(io, &self.controller.store)?
                .as_ref()
                .map(profile::capture)
                .transpose()
        })
    }
    pub(crate) fn preview_profile(
        &self,
        hal: &Hal,
        target: &profile::Target,
    ) -> Result<profile::Review> {
        target.validate()?;
        self.with_io(hal, |io| {
            let p = preview(io, &self.controller.store)?.context("本机未提供档案显卡模式接口")?;
            let current = profile::capture(&p)?;
            if current.owner != target.owner || current.available_mask != target.available_mask {
                bail!("档案显卡模式所属Windows或固件候选已变化");
            }
            let before = describe(&p);
            transaction::Plan::new(p, target.mode)?;
            Ok(profile::Review {
                before,
                target: target.mode,
                software_fan_running: crate::fanctl::software::read(hal)?.is_some(),
            })
        })
    }
    pub(crate) fn execute_profile(&mut self, hal: &Hal, review: &profile::Review) -> Result<()> {
        if self.active.is_some() {
            bail!("档案显卡选择已执行");
        }
        let mut plan = self.with_io(hal, |io| {
            let p = preview(io, &self.controller.store)?.context("显卡模式接口已不可用")?;
            check_revision(&p, &review.before.revision)?;
            transaction::Plan::new(p, review.target)
        })?;
        let result = self.with_io(hal, |io| plan.execute(io, &self.controller.store));
        self.active = Some(plan);
        result
    }
    pub(crate) fn rollback_profile(&mut self, hal: &Hal) -> Result<()> {
        let Some(mut plan) = self.active.take() else {
            return Ok(());
        };
        let result = self.with_io(hal, |io| plan.rollback(io, &self.controller.store));
        self.active = Some(plan);
        result
    }
}
