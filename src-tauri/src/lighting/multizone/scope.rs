use super::*;
#[cfg(test)]
use std::sync::Arc;
use std::sync::{Mutex, MutexGuard};

#[derive(Default)]
pub struct Controller {
    baseline: Mutex<Option<Snapshot>>,
    #[cfg(test)]
    io: Option<Arc<dyn Io + Send + Sync>>,
}
pub(crate) struct Scope<'a> {
    controller: &'a Controller,
    baseline: MutexGuard<'a, Option<Snapshot>>,
}
pub(crate) struct Plan {
    before: Snapshot,
    target: Target,
}
impl Controller {
    #[cfg(test)]
    pub(crate) fn with_test_io(mut self, io: Arc<dyn Io + Send + Sync>) -> Self {
        self.io = Some(io);
        self
    }
    pub(crate) fn scope(&self) -> Result<Scope<'_>> {
        Ok(Scope {
            controller: self,
            baseline: self
                .baseline
                .lock()
                .map_err(|_| anyhow!("多区键盘恢复状态异常"))?,
        })
    }
    fn with_io<T>(&self, hal: &Hal, f: impl FnOnce(&dyn Io) -> Result<T>) -> Result<Option<T>> {
        #[cfg(test)]
        {
            if let Some(io) = &self.io {
                return f(io.as_ref()).map(Some);
            }
            if hal.with_dchu(native::supported)? {
                bail!("隔离多区键盘缺少显式IO，未访问原厂提供者");
            }
            Ok(None)
        }
        #[cfg(not(test))]
        hal.with_dchu(|dchu| {
            if native::supported(dchu)? {
                f(&native::Native(dchu)).map(Some)
            } else {
                Ok(None)
            }
        })
    }
    pub fn read(&self, hal: &Hal) -> Result<Option<Profile>> {
        let scope = self.scope()?;
        let _gate = hal.begin_write()?;
        scope.read(hal)
    }
    pub fn apply(&self, hal: &Hal, request: &Request) -> Result<()> {
        let mut scope = self.scope()?;
        let _gate = hal.begin_write()?;
        scope.apply(hal, request, &mut false)
    }
    pub fn restore(&self, hal: &Hal, expected: &str) -> Result<()> {
        let scope = self.scope()?;
        let _gate = hal.begin_write()?;
        self.with_io(hal, |io| {
            io.ensure_available()?;
            let current = io.snapshot()?;
            current.validate()?;
            if current.revision() != expected {
                bail!("多区键盘状态已变化，请重新确认恢复");
            }
            restore(io, scope.baseline.as_ref().context("没有多区键盘初始快照")?)
        })?
        .context("多区键盘不可用")
    }
}
impl Scope<'_> {
    pub(crate) fn read(&self, hal: &Hal) -> Result<Option<Profile>> {
        let profile = self.controller.with_io(hal, |io| {
            let blocked = io
                .ensure_available()
                .err()
                .map(|error| format!("{error:#}"));
            io.snapshot()?.profile(&self.baseline, blocked)
        })?;
        if profile.is_none() && self.baseline.is_some() {
            bail!("多区键盘能力已变化，初始恢复记录已保留");
        }
        Ok(profile)
    }
    pub(crate) fn capture(&self, hal: &Hal) -> Result<Option<Target>> {
        self.controller.with_io(hal, |io| {
            let before = io.snapshot()?;
            Ok(Target {
                values: before.values()?,
                capabilities: before.capabilities,
            })
        })
    }
    pub(crate) fn prepare(&self, hal: &Hal, target: &Target) -> Result<Plan> {
        target.validate()?;
        self.controller
            .with_io(hal, |io| {
                io.ensure_available()?;
                let before = io.snapshot()?;
                before.values()?;
                before.target(target)?;
                if let Some(initial) = self.baseline.as_ref() {
                    before.require_device(initial)?;
                }
                Ok(Plan {
                    before,
                    target: target.clone(),
                })
            })?
            .context("档案多区键盘不可用")
    }
    pub(super) fn apply(&mut self, hal: &Hal, request: &Request, dirty: &mut bool) -> Result<()> {
        self.controller
            .with_io(hal, |io| {
                super::apply(io, &mut self.baseline, request, dirty)
            })?
            .context("多区键盘不可用")
    }
    pub(crate) fn execute(&mut self, hal: &Hal, plan: &Plan, dirty: &mut bool) -> Result<()> {
        self.apply(
            hal,
            &Request {
                expected_revision: plan.before.revision(),
                target: plan.target.clone(),
            },
            dirty,
        )
    }
    pub(crate) fn rollback(&self, hal: &Hal, plan: &Plan) -> Result<()> {
        self.controller
            .with_io(hal, |io| {
                io.ensure_available()?;
                restore(io, &plan.before)
            })?
            .context("恢复本次多区键盘时原能力已不可用")
    }
}
