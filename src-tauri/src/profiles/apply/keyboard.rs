use super::*;

pub(super) struct Plan {
    keyboard: Option<crate::keyboard::Plan>,
    lighting: Option<crate::lighting::Plan>,
    multi_zone: Option<crate::lighting::multizone::Plan>,
    options: Option<crate::lighting::options::Plan>,
    perkey: Option<crate::lighting::perkey::Plan>,
    keymap: Option<crate::keymap::Plan>,
}
#[derive(Default)]
pub(super) struct Touched {
    keyboard: bool,
    lighting: bool,
    multi_zone: bool,
    options: bool,
    perkey: bool,
    keymap: bool,
}
impl Plan {
    pub(super) fn prepare(
        hal: &Hal,
        scopes: &mut Scopes<'_>,
        settings: &ProfileSettings,
    ) -> Result<Self> {
        Ok(Self {
            multi_zone: settings
                .multi_zone_lighting
                .as_ref()
                .map(|target| scopes.multi_zone.prepare(hal, target))
                .transpose()?,
            keymap: settings
                .keymap
                .as_ref()
                .map(|target| scopes.keymap.prepare(hal, target))
                .transpose()?,
            keyboard: settings
                .keyboard
                .as_ref()
                .map(|value| scopes.keyboard.prepare(hal, value))
                .transpose()?,
            lighting: settings
                .keyboard_lighting
                .as_ref()
                .map(|value| scopes.lighting.prepare(hal, value))
                .transpose()?,
            options: settings
                .lighting_options
                .as_ref()
                .map(|value| scopes.lighting_options.prepare(hal, value))
                .transpose()?,
            perkey: settings
                .perkey_lighting
                .as_ref()
                .map(|target| scopes.perkey.prepare(hal, target))
                .transpose()?,
        })
    }
    pub(super) fn execute(
        &self,
        hal: &Hal,
        scopes: &mut Scopes<'_>,
        touched: &mut Touched,
        steps: &mut Vec<ApplyStep>,
        review: ModeReview<'_>,
    ) -> Result<()> {
        if let Some(plan) = &self.keyboard {
            guard(steps, review)?;
            stage(
                steps,
                "键盘功能",
                scopes.keyboard.execute(hal, plan, &mut touched.keyboard),
            )?;
        }
        if let Some(plan) = &self.lighting {
            guard(steps, review)?;
            stage(
                steps,
                "键盘灯效",
                scopes.lighting.execute(hal, plan, &mut touched.lighting),
            )?;
        }
        if let Some(plan) = &self.perkey {
            guard(steps, review)?;
            stage(
                steps,
                "逐键灯效",
                scopes.perkey.execute(hal, plan, &mut touched.perkey),
            )?;
        }
        if let Some(plan) = &self.multi_zone {
            guard(steps, review)?;
            stage(
                steps,
                "多区键盘灯效",
                scopes
                    .multi_zone
                    .execute(hal, plan, &mut touched.multi_zone),
            )?;
        }
        if let Some(plan) = &self.options {
            guard(steps, review)?;
            stage(
                steps,
                "灯光策略与外围灯区",
                scopes
                    .lighting_options
                    .execute(hal, plan, &mut touched.options),
            )?;
        }
        if let Some(plan) = &self.keymap {
            guard(steps, review)?;
            stage(
                steps,
                "按键映射与宏",
                scopes.keymap.execute(hal, plan, &mut touched.keymap),
            )?;
        }
        Ok(())
    }
    pub(super) fn rollback(
        &self,
        hal: &Hal,
        scopes: &mut Scopes<'_>,
        touched: &Touched,
        steps: &mut Vec<ApplyStep>,
    ) {
        if let Some(plan) = self.keymap.as_ref().filter(|_| touched.keymap) {
            let _ = stage(steps, "恢复按键映射与宏", scopes.keymap.rollback(hal, plan));
        }
        if let Some(plan) = self.multi_zone.as_ref().filter(|_| touched.multi_zone) {
            let _ = stage(
                steps,
                "恢复多区键盘灯效",
                scopes.multi_zone.rollback(hal, plan),
            );
        }
        if let Some(plan) = self.perkey.as_ref().filter(|_| touched.perkey) {
            let _ = stage(steps, "恢复逐键灯效", scopes.perkey.rollback(hal, plan));
        }
        if let Some(plan) = self.lighting.as_ref().filter(|_| touched.lighting) {
            let _ = stage(steps, "恢复键盘灯效", scopes.lighting.rollback(hal, plan));
        }
        if let Some(plan) = self.options.as_ref().filter(|_| touched.options) {
            let _ = stage(
                steps,
                "恢复灯光策略与外围灯区",
                scopes.lighting_options.rollback(hal, plan),
            );
        }
        if let Some(plan) = self.keyboard.as_ref().filter(|_| touched.keyboard) {
            let _ = stage(steps, "恢复键盘功能", scopes.keyboard.rollback(hal, plan));
        }
    }
    pub(super) fn prepare_sessions(&self, hal: &Hal, scopes: &mut Scopes<'_>) -> Result<()> {
        if let Some(plan) = &self.perkey {
            scopes.perkey.prepare_candidate(hal, plan)?;
        }
        if let Some(plan) = &self.keymap {
            scopes.keymap.prepare_candidate(hal, plan)?;
        }
        Ok(())
    }
    pub(super) fn commit(
        &self,
        hal: &Hal,
        scopes: &mut Scopes<'_>,
        steps: &mut Vec<ApplyStep>,
    ) -> Result<()> {
        if let Some(plan) = &self.keymap {
            stage(steps, "提交映射与程序监听", scopes.keymap.commit(hal, plan))?;
        }
        Ok(())
    }
}
