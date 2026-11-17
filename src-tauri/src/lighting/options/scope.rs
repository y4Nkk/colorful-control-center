use super::*;
use std::sync::MutexGuard;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Target {
    pub keyboard_type: u8,
    pub values: Values,
}
impl Target {
    pub fn validate(&self) -> Result<()> {
        if !matches!(
            family_from_type(self.keyboard_type),
            KeyboardFamily::White
                | KeyboardFamily::ZoneRgb
                | KeyboardFamily::Perkey
                | KeyboardFamily::MultiZone
        ) {
            bail!("灯光策略键盘型号未知");
        }
        self.values.validate()
    }
}
pub(crate) struct Scope<'a> {
    pub(super) baseline: MutexGuard<'a, Option<Snapshot>>,
}
pub(crate) struct Plan {
    before: Snapshot,
    values: Values,
    parts: Vec<transaction::Part>,
}
impl Controller {
    pub(crate) fn scope(&self) -> Result<Scope<'_>> {
        Ok(Scope {
            baseline: self
                .baseline
                .lock()
                .map_err(|_| anyhow!("灯光恢复状态异常"))?,
        })
    }
}
impl Scope<'_> {
    pub(crate) fn capture(&self, hal: &Hal) -> Result<Option<Target>> {
        hal.with_dchu(|io| {
            let family = super::super::build_profile(io, None).detected_family;
            if family == KeyboardFamily::None {
                return Ok(None);
            }
            let snapshot = capture(io)?;
            Ok(Some(Target {
                keyboard_type: snapshot.caps.keyboard_type,
                values: snapshot.values()?,
            }))
        })
    }
    pub(crate) fn prepare(&self, hal: &Hal, target: &Target) -> Result<Plan> {
        target.validate()?;
        hal.with_dchu(|io| {
            let before = capture(io)?;
            if before.caps.keyboard_type != target.keyboard_type {
                bail!("灯光策略键盘型号已变化");
            }
            if let Some(initial) = self.baseline.as_ref() {
                before.require_same_device(initial)?;
            }
            let after = before.target(&target.values)?;
            let parts = transaction::changes(&before, &after)?;
            transaction::commands(&after, &parts)?;
            transaction::commands(&before, &parts)?;
            Ok(Plan {
                before,
                values: target.values.clone(),
                parts,
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
            let current = capture(io)?;
            current.require_same_device(&plan.before)?;
            if current.values()? != plan.before.values()? {
                bail!("灯光策略保存值已变化，请重新预览档案");
            }
            transaction::apply_tracked(
                io,
                &mut self.baseline,
                &Request {
                    expected_revision: current.revision(),
                    values: plan.values.clone(),
                },
                needs_rollback,
            )
        })
    }
    pub(crate) fn rollback(&self, hal: &Hal, plan: &Plan) -> Result<()> {
        hal.with_dchu(|io| transaction::restore_parts(io, &plan.before, &plan.parts))
    }
}
