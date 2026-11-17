use super::*;

fn identity(profile: &Profile) -> Result<String> {
    profile.capabilities.validate()?;
    profile.current.as_ref().context("多区键盘保存值未知")?;
    if let Some(reason) = &profile.blocked_reason {
        bail!("{reason}");
    }
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&(
            profile.capabilities.keyboard_type,
            profile.capabilities.light_bar_switchable,
            profile.capabilities.cover,
        ))?)
    ))
}
impl Controller {
    pub(crate) fn toggle_enabled(&self, hal: &Hal, expected: &str) -> Result<bool> {
        let mut scope = self.scope()?;
        let _gate = hal.begin_write()?;
        let profile = scope.read(hal)?.context("多区键盘不可用")?;
        if identity(&profile)? != expected {
            bail!("多区键盘授权能力已变化");
        }
        let mut values = profile.current.context("多区键盘保存值未知")?;
        values.enabled = !values.enabled;
        let enabled = values.enabled;
        scope.apply(
            hal,
            &Request {
                expected_revision: profile.revision,
                target: Target {
                    capabilities: profile.capabilities,
                    values,
                },
            },
            &mut false,
        )?;
        Ok(enabled)
    }
    pub(crate) fn hotkey_identity(&self, hal: &Hal) -> Result<String> {
        let scope = self.scope()?;
        let _gate = hal.begin_write()?;
        identity(&scope.read(hal)?.context("多区键盘不可用")?)
    }
    pub(crate) fn adjust_brightness(&self, hal: &Hal, expected: &str, delta: i8) -> Result<u8> {
        let mut scope = self.scope()?;
        let _gate = hal.begin_write()?;
        let profile = scope.read(hal)?.context("多区键盘不可用")?;
        if identity(&profile)? != expected {
            bail!("多区键盘授权能力已变化");
        }
        let mut values: Values = profile.current.context("多区键盘保存值未知")?;
        values.brightness =
            (i16::from(values.brightness) + i16::from(delta.signum())).clamp(1, 4) as u8;
        values.enabled = true;
        let level = values.brightness;
        scope.apply(
            hal,
            &Request {
                expected_revision: profile.revision,
                target: Target {
                    capabilities: profile.capabilities,
                    values,
                },
            },
            &mut false,
        )?;
        Ok(level)
    }
}
