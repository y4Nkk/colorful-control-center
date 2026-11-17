use super::*;
#[cfg(test)]
use std::sync::Arc;
use std::sync::{Mutex, MutexGuard};

#[derive(Default)]
pub struct Controller {
    baseline: Mutex<Option<Snapshot>>,
    #[cfg(test)]
    test_io: Option<Arc<dyn Io + Send + Sync>>,
}
pub(crate) struct Scope<'a> {
    controller: &'a Controller,
    baseline: MutexGuard<'a, Option<Snapshot>>,
}
pub(crate) struct Plan {
    before: Snapshot,
    values: Values,
}
impl Controller {
    #[cfg(test)]
    pub(crate) fn with_test_io(mut self, io: Arc<dyn Io + Send + Sync>) -> Self {
        self.test_io = Some(io);
        self
    }
    pub(crate) fn scope(&self) -> Result<Scope<'_>> {
        Ok(Scope {
            controller: self,
            baseline: self
                .baseline
                .lock()
                .map_err(|_| anyhow!("灯条恢复状态异常"))?,
        })
    }
    fn with_io<T>(
        &self,
        hal: &Hal,
        action: impl FnOnce(&dyn Io) -> Result<T>,
    ) -> Result<Option<T>> {
        #[cfg(test)]
        {
            if let Some(io) = &self.test_io {
                return action(io.as_ref()).map(Some);
            }
            let kinds = hal.with_dchu(|io| model::kinds(&io.read_bios_capabilities(0, 24)?))?;
            if !kinds.is_empty() {
                bail!("隔离测试未注入灯条传输，未访问真实HID");
            }
            Ok(None)
        }
        #[cfg(not(test))]
        hal.with_dchu(|dchu| match native::open(dchu)? {
            Some(io) => action(&io).map(Some),
            None => Ok(None),
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
        self.with_io(hal, |io| restore_initial(io, &scope.baseline, expected))?
            .context("固件未声明可用的独立灯条")
    }
}
impl Scope<'_> {
    pub(crate) fn read(&self, hal: &Hal) -> Result<Option<Profile>> {
        let result = self.controller.with_io(hal, |io| {
            let blocked = io
                .ensure_available()
                .err()
                .map(|error| format!("{error:#}"));
            profile(&io.snapshot()?, &self.baseline, blocked)
        })?;
        if result.is_none() && self.baseline.is_some() {
            bail!("灯条能力已丢失，初始快照已保留");
        }
        Ok(result)
    }
    pub(crate) fn capture(&self, hal: &Hal) -> Result<Option<Values>> {
        self.controller.with_io(hal, |io| io.snapshot()?.values())
    }
    pub(crate) fn prepare(&self, hal: &Hal, values: &Values) -> Result<Plan> {
        self.controller
            .with_io(hal, |io| {
                io.ensure_available()?;
                let before = io.snapshot()?;
                if self
                    .baseline
                    .as_ref()
                    .is_some_and(|initial| !initial.same_device(&before))
                {
                    bail!("灯条设备已变化，初始恢复记录已保留");
                }
                before.target(values)?;
                Ok(Plan {
                    before,
                    values: values.clone(),
                })
            })?
            .context("档案中的独立灯条不可用")
    }
    fn apply(&mut self, hal: &Hal, request: &Request, needs_rollback: &mut bool) -> Result<()> {
        let controller = self.controller;
        controller
            .with_io(hal, |io| {
                apply_tracked(io, &mut self.baseline, request, needs_rollback)
            })?
            .context("固件未声明可用的独立灯条")
    }
    pub(crate) fn execute(
        &mut self,
        hal: &Hal,
        plan: &Plan,
        needs_rollback: &mut bool,
    ) -> Result<()> {
        self.apply(
            hal,
            &Request {
                expected_revision: plan.before.revision(),
                values: plan.values.clone(),
            },
            needs_rollback,
        )
    }
    pub(crate) fn rollback(&self, hal: &Hal, plan: &Plan) -> Result<()> {
        self.controller
            .with_io(hal, |io| {
                io.ensure_available()?;
                if !io.snapshot()?.same_device(&plan.before) {
                    bail!("灯条设备已变化，未向新设备恢复档案前态");
                }
                restore(io, &plan.before)
            })?
            .context("恢复档案前灯条配置时原设备不可用")
    }
}
