use super::*;
use std::sync::MutexGuard;

pub(crate) struct Scope<'a> {
    pub(super) baseline: MutexGuard<'a, Option<Snapshot>>,
}

impl Controller {
    pub(crate) fn scope(&self) -> Result<Scope<'_>> {
        Ok(Scope {
            baseline: self
                .baseline
                .lock()
                .map_err(|_| anyhow!("PD恢复状态异常"))?,
        })
    }
}

impl Scope<'_> {
    pub fn capture(&self, hal: &Hal) -> Result<Option<Target>> {
        hal.with_dchu(|io| {
            if !io.capabilities()?.pd {
                return Ok(None);
            }
            let current = capture(io)?;
            current.validate_current()?;
            Ok(Some(current.profile_target()))
        })
    }

    pub fn prepare(&self, hal: &Hal, target: &Target) -> Result<Plan> {
        hal.ensure_mode_change_allowed()?;
        hal.with_dchu(|io| {
            let current = capture(io)?;
            current.require_target(target)?;
            Plan::prepare(current, &self.baseline, target.level)
        })
    }

    pub fn execute(&mut self, hal: &Hal, plan: &Plan, touched: &mut bool) -> Result<()> {
        hal.ensure_mode_change_allowed()?;
        hal.with_dchu(|io| plan.execute(io, &mut self.baseline, touched))
    }

    pub fn rollback(&self, hal: &Hal, plan: &Plan) -> Result<()> {
        hal.ensure_mode_change_allowed()?;
        hal.with_dchu(|io| plan.rollback(io))
    }
}
