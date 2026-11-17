use super::*;
use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};
use std::sync::MutexGuard;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Target {
    pub keyboard_type: u8,
    pub enabled: bool,
    pub settings: LightingRequest,
}
impl Target {
    pub fn validate(&self) -> Result<()> {
        if family_from_type(self.keyboard_type) != self.settings.family() {
            bail!("键盘灯型号与设置类型不一致");
        }
        self.settings.validate()
    }
    pub(super) fn patches(&self) -> Vec<Patch> {
        self.settings
            .patches()
            .into_iter()
            .map(|mut patch| {
                if patch.offset == 80 {
                    patch.data = vec![u8::from(self.enabled)];
                }
                patch
            })
            .collect()
    }
}
pub(crate) struct Scope<'a> {
    pub(super) baseline: MutexGuard<'a, Option<Baseline>>,
}
pub(crate) struct Plan {
    before: Baseline,
    target: Target,
}
impl LightingController {
    pub(crate) fn scope(&self) -> Result<Scope<'_>> {
        Ok(Scope {
            baseline: self
                .baseline
                .lock()
                .map_err(|_| anyhow!("键盘灯控制状态异常"))?,
        })
    }
}
fn detected(hal: &Hal) -> Result<LightingProfile> {
    let profile = hal.with_dchu(|d| Ok(build_profile(d, None)))?;
    if !profile.available {
        bail!(
            profile
                .unavailable_reason
                .unwrap_or_else(|| "灯光通道不可用".into())
        );
    }
    if profile.detected_family == KeyboardFamily::Unknown {
        bail!("键盘类型未知，无法捕获灯效");
    }
    Ok(profile)
}
pub(super) fn require_device(
    hal: &Hal,
    raw_type: Option<u8>,
    family: KeyboardFamily,
) -> Result<()> {
    let current = detected(hal)?;
    if current.detected_type != raw_type || current.detected_family != family {
        bail!("键盘灯型号已变化，拒绝向其他设备写入");
    }
    Ok(())
}
impl Scope<'_> {
    pub(crate) fn capture(&self, hal: &Hal) -> Result<Option<Target>> {
        let profile = detected(hal)?;
        if matches!(
            profile.detected_family,
            KeyboardFamily::None | KeyboardFamily::Perkey | KeyboardFamily::MultiZone
        ) {
            return Ok(None);
        }
        let block = DchuLighting(hal).read_saved_block()?;
        Ok(Some(Target {
            keyboard_type: profile.detected_type.context("键盘型号未知")?,
            enabled: block.snapshot().enabled,
            settings: block.request(profile.detected_family)?,
        }))
    }
    pub(crate) fn prepare(&self, hal: &Hal, target: &Target) -> Result<Plan> {
        target.validate()?;
        require_device(hal, Some(target.keyboard_type), target.settings.family())?;
        let before = transaction::capture(
            &DchuLighting(hal),
            target.settings.family(),
            Some(target.keyboard_type),
        )
        .map_err(|e| anyhow!(e.message))?;
        transaction::require_baseline(&self.baseline, &before).map_err(|e| anyhow!(e.message))?;
        Ok(Plan {
            before,
            target: target.clone(),
        })
    }
    pub(crate) fn execute(
        &mut self,
        hal: &Hal,
        plan: &Plan,
        needs_rollback: &mut bool,
    ) -> Result<()> {
        require_device(hal, plan.before.raw_type, plan.before.family)?;
        let io = DchuLighting(hal);
        if io.read_saved_block()?.bytes != plan.before.block {
            bail!("键盘灯保存值已变化，请重新预览档案");
        }
        transaction::apply_target(&io, &mut self.baseline, &plan.target, needs_rollback).map_err(
            |error| {
                anyhow!(format!(
                    "{}{}",
                    error.message,
                    error.rollback.map(|e| format!("；{e}")).unwrap_or_default()
                ))
            },
        )
    }
    pub(crate) fn rollback(&self, hal: &Hal, plan: &Plan) -> Result<()> {
        require_device(hal, plan.before.raw_type, plan.before.family)?;
        transaction::restore_snapshot(&DchuLighting(hal), &plan.before)
    }
}
