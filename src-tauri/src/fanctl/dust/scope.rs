use super::*;
use std::sync::{Mutex, MutexGuard};

#[derive(Default)]
pub struct Controller {
    state: Mutex<State>,
}
pub(crate) struct Scope<'a> {
    state: MutexGuard<'a, State>,
}
pub(crate) struct Plan {
    before: Facts,
    schedule: Schedule,
}
impl Plan {
    pub(crate) fn disables_schedule(&self) -> bool {
        !self.schedule.enabled()
    }
    pub(crate) fn restores_before_fan(&self) -> bool {
        protocol::decode(&self.before.saved)
            .is_ok_and(|schedule| schedule.is_some_and(|s| !s.enabled()))
    }
}
impl Controller {
    pub(crate) fn scope(&self) -> Result<Scope<'_>> {
        Ok(Scope {
            state: self.state.lock().map_err(|_| anyhow!("除尘状态异常"))?,
        })
    }
    pub fn read(&self, hal: &Hal) -> Result<Option<Profile>> {
        let mut scope = self.scope()?;
        let _gate = hal.begin_write()?;
        let software = super::super::software::active(hal)?;
        let blocked = hal
            .ensure_mode_change_allowed()
            .err()
            .map(|e| format!("{e:#}"));
        hal.with_dchu(|io| {
            read(io, &mut scope.state, software, blocked).map(|value| value.map(|(_, p)| p))
        })
    }
    pub fn start(&self, hal: &Hal, expected: &str) -> Result<()> {
        let mut scope = self.scope()?;
        let _gate = hal.begin_write()?;
        hal.ensure_mode_change_allowed()?;
        let software = super::super::software::active(hal)?;
        hal.with_dchu(|io| start(io, &mut scope.state, software, expected))
    }
    pub fn apply(&self, hal: &Hal, request: &Request) -> Result<()> {
        let mut scope = self.scope()?;
        let _gate = hal.begin_write()?;
        hal.ensure_mode_change_allowed()?;
        let software = super::super::software::active(hal)?;
        hal.with_dchu(|io| apply(io, &mut scope.state, software, request))
    }
    pub fn restore(&self, hal: &Hal, expected: &str) -> Result<()> {
        let mut scope = self.scope()?;
        let _gate = hal.begin_write()?;
        hal.ensure_mode_change_allowed()?;
        let software = super::super::software::active(hal)?;
        hal.with_dchu(|io| restore(io, &mut scope.state, software, expected))
    }
}
impl Scope<'_> {
    pub(crate) fn capture(&self, hal: &Hal) -> Result<Option<Schedule>> {
        hal.with_dchu(|io| {
            let Some(facts) = capture(io)? else {
                return Ok(None);
            };
            Ok(Some(protocol::decode(&facts.saved)?.context(
                "原厂未报告除尘计划，无法捕获；请先明确保存计划",
            )?))
        })
    }
    pub(crate) fn prepare(
        &self,
        hal: &Hal,
        schedule: Schedule,
        software_after: bool,
    ) -> Result<Plan> {
        schedule.validate()?;
        hal.ensure_mode_change_allowed()?;
        if software_after && schedule.enabled() {
            bail!("软件风扇曲线运行时不能启用除尘计划");
        }
        hal.with_dchu(|io| {
            let before = capture(io)?.context("档案中的除尘计划不可用")?;
            if before.running {
                bail!("固件正在除尘，请等待结束后再应用档案");
            }
            Ok(Plan { before, schedule })
        })
    }
    pub(crate) fn execute(
        &mut self,
        hal: &Hal,
        plan: &Plan,
        needs_rollback: &mut bool,
    ) -> Result<()> {
        hal.ensure_mode_change_allowed()?;
        if plan.schedule.enabled() && super::super::software::active(hal)? {
            bail!("软件风扇曲线仍运行，未启用除尘计划");
        }
        hal.with_dchu(|io| {
            let current = capture(io)?.context("档案中的除尘能力已丢失")?;
            if current.saved != plan.before.saved || current.running {
                bail!("除尘计划或运行状态已变化，请重新预览档案");
            }
            transaction::apply_tracked(
                io,
                &mut self.state,
                &current,
                plan.schedule.saved(),
                needs_rollback,
            )
        })
    }
    pub(crate) fn rollback(&self, hal: &Hal, plan: &Plan) -> Result<()> {
        hal.ensure_mode_change_allowed()?;
        if !plan.restores_before_fan() && super::super::software::active(hal)? {
            bail!("软件风扇曲线尚未退出，除尘恢复快照保留待重试");
        }
        hal.with_dchu(|io| transaction::recover(io, &plan.before.saved))
    }
}
