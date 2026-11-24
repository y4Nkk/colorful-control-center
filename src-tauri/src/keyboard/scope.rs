use super::*;
use std::sync::MutexGuard;

pub(crate) struct Scope<'a> {
    _lock: MutexGuard<'a, ()>,
}
pub(crate) struct Plan {
    before: Snapshot,
    values: Values,
}
impl Controller {
    pub(crate) fn scope(&self) -> Result<Scope<'_>> {
        Ok(Scope {
            _lock: self
                .mutation
                .lock()
                .map_err(|_| anyhow!("键盘控制状态异常"))?,
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
            target(&before, values)?;
            Ok(Plan {
                before,
                values: values.clone(),
            })
        })
    }
    pub(crate) fn execute(
        &self,
        hal: &Hal,
        plan: &Plan,
        needs_rollback: &mut bool,
    ) -> Result<()> {
        hal.with_dchu(|io| {
            apply_tracked(
                io,
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
