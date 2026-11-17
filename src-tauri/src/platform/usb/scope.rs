use super::*;
use model::Change;
use std::sync::MutexGuard;

pub(crate) struct Scope<'a> {
    pub(super) controller: &'a Controller,
    pub(super) baseline: MutexGuard<'a, Baseline>,
}
pub(crate) struct Plan {
    before: Frame,
    changes: Vec<Change>,
    active_after_mode: u128,
}
impl Controller {
    pub(crate) fn scope(&self) -> Result<Scope<'_>> {
        Ok(Scope {
            controller: self,
            baseline: self
                .baseline
                .lock()
                .map_err(|_| anyhow!("USB恢复快照异常"))?,
        })
    }
    pub(super) fn with_io<T>(&self, hal: &Hal, f: impl FnOnce(&dyn Io) -> Result<T>) -> Result<T> {
        #[cfg(test)]
        {
            let _ = hal;
            f(self.io.as_deref().context("隔离USB控制器未注入IO")?)
        }
        #[cfg(not(test))]
        f(&Native(hal))
    }
    #[cfg(test)]
    pub(crate) fn test(io: std::sync::Arc<dyn Io + Send + Sync>) -> Self {
        Self {
            baseline: Mutex::default(),
            io: Some(io),
        }
    }
}
impl Scope<'_> {
    pub(super) fn with_io<T>(&self, hal: &Hal, f: impl FnOnce(&dyn Io) -> Result<T>) -> Result<T> {
        self.controller.with_io(hal, f)
    }
    pub fn capture(&self, hal: &Hal) -> Result<Option<Target>> {
        self.with_io(hal, |io| {
            let Some(frame) = capture(io, false)? else {
                return Ok(None);
            };
            let target = frame.profile_target();
            target.validate()?;
            Ok(Some(target))
        })
    }
    pub fn prepare(
        &self,
        hal: &Hal,
        target: &Target,
        active_after_mode: Option<u128>,
    ) -> Result<Plan> {
        hal.ensure_mode_change_allowed()?;
        self.with_io(hal, |io| {
            let before = capture(io, false)?.context("固件未声明USB策略")?;
            let changes = before.changes_for(target)?;
            io.ensure_write()?;
            let active_after_mode = active_after_mode.unwrap_or(before.active);
            if !io.power().schemes()?.contains(&active_after_mode) {
                bail!("模式目标的电源计划不存在");
            }
            Ok(Plan {
                before,
                changes,
                active_after_mode,
            })
        })
    }
    pub fn execute(&mut self, hal: &Hal, plan: &Plan, touched: &mut Vec<Change>) -> Result<()> {
        hal.ensure_mode_change_allowed()?;
        self.controller.with_io(hal, |io| {
            let current = capture(io, false)?.context("USB能力已丢失")?;
            if current.firmware != plan.before.firmware
                || current.plans != plan.before.plans
                || current.active != plan.active_after_mode
            {
                bail!("USB计划或字段已变化，未应用档案");
            }
            transaction::execute(io, &mut self.baseline, &current, &plan.changes, touched)
        })
    }
    pub fn rollback(&self, hal: &Hal, touched: &[Change]) -> Result<()> {
        hal.ensure_mode_change_allowed()?;
        self.with_io(hal, |io| {
            io.ensure_write()?;
            transaction::compensate(io.power(), touched)
        })
    }
}
