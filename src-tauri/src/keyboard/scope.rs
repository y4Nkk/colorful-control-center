use super::*;
use std::sync::MutexGuard;

pub(crate) struct Scope<'a> {
    pub(super) baseline: MutexGuard<'a, Option<Snapshot>>,
}
pub(crate) struct Plan {
    before: Snapshot,
    values: Values,
}
impl Controller {
    pub(crate) fn scope(&self) -> Result<Scope<'_>> {
        Ok(Scope {
            baseline: self
                .baseline
                .lock()
                .map_err(|_| anyhow!("键盘恢复状态异常"))?,
        })
    }
}
impl Scope<'_> {
    pub(crate) fn capture(&self, hal: &Hal) -> Result<Option<Values>> {
        hal.with_dchu(|io| {
            let values = read(io)?.values();
            Ok((!values.is_empty()).then_some(values))
        })
    }
    pub(crate) fn prepare(&self, hal: &Hal, values: &Values) -> Result<Plan> {
        if values.is_empty() {
            bail!("档案中的键盘设置为空");
        }
        hal.with_dchu(|io| {
            let before = read(io)?;
            if let Some(original) = self.baseline.as_ref() {
                require_same_capabilities(&before, original)?;
            }
            target(&before, values)?;
            Ok(Plan {
                before,
                values: values.clone(),
            })
        })
    }
    pub(crate) fn execute(
        &mut self,
        hal: &Hal,
        plan: &Plan,
        needs_rollback: &mut bool,
    ) -> Result<()> {
        hal.with_dchu(|io| {
            apply_tracked(
                io,
                &mut self.baseline,
                &Request {
                    expected_revision: plan.before.revision(),
                    values: plan.values.clone(),
                },
                needs_rollback,
            )
        })
    }
    pub(crate) fn rollback(&self, hal: &Hal, plan: &Plan) -> Result<()> {
        hal.with_dchu(|io| restore(io, &plan.before))
    }
}
