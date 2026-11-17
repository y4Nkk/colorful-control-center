use super::*;
pub(crate) fn navigation_identity(profile: &LightingProfile) -> Result<String> {
    let raw = profile
        .detected_type
        .ok_or_else(|| anyhow::anyhow!("键盘类型未知"))?;
    if !ccc_core::oem_events::keyboard_navigation_type_allowed(raw) {
        anyhow::bail!("当前键盘没有原厂键盘灯快捷入口");
    }
    Ok(format!("keyboard-panel:{raw}"))
}
use anyhow::Context;

pub(crate) fn identity(profile: &LightingProfile) -> Result<String> {
    if !profile.available {
        anyhow::bail!(
            profile
                .unavailable_reason
                .clone()
                .unwrap_or("键盘灯不可用".into())
        );
    }
    if !matches!(
        profile.detected_family,
        KeyboardFamily::White | KeyboardFamily::ZoneRgb
    ) {
        anyhow::bail!("当前键盘不是普通RGB或白光");
    }
    if profile.current.is_none() {
        anyhow::bail!("键盘灯保存状态未知");
    }
    Ok(format!(
        "{:?}:{:?}",
        profile.detected_family, profile.detected_type
    ))
}
impl LightingController {
    pub(crate) fn toggle_enabled(&self, hal: &Hal, expected: &str) -> Result<bool> {
        let mut scope = self.scope()?;
        let _gate = hal.begin_write()?;
        let profile = hal.with_dchu(|d| Ok(build_profile(d, None)))?;
        if identity(&profile)? != expected {
            anyhow::bail!("键盘类型已变化，请重新确认热键");
        }
        let mut target = scope.capture(hal)?.context("键盘灯保存状态未知")?;
        target.enabled = !target.enabled;
        let plan = scope.prepare(hal, &target)?;
        scope.execute(hal, &plan, &mut false)?;
        Ok(target.enabled)
    }
    pub(crate) fn adjust_brightness(&self, hal: &Hal, expected: &str, delta: i8) -> Result<u8> {
        let mut baseline = self
            .baseline
            .lock()
            .map_err(|_| anyhow!("键盘灯控制状态异常"))?;
        let _gate = hal.begin_write()?;
        let profile = hal.with_dchu(|d| Ok(build_profile(d, None)))?;
        if identity(&profile)? != expected {
            anyhow::bail!("键盘类型已变化，请重新确认热键");
        }
        let io = DchuLighting(hal);
        let mut request = io.read_saved_block()?.request(profile.detected_family)?;
        let level = match &mut request {
            LightingRequest::White { brightness } => {
                *brightness = step(*brightness, delta, 5);
                *brightness
            }
            LightingRequest::ZoneRgb { brightness, .. } => {
                *brightness = step(*brightness, delta, 4);
                *brightness
            }
        };
        let outcome = self.apply_io(&io, &mut baseline, &request, profile.detected_type);
        if !outcome.ok {
            anyhow::bail!(
                "{}{}",
                outcome.message,
                outcome
                    .rollback_error
                    .map(|e| format!("；{e}"))
                    .unwrap_or_default()
            );
        }
        Ok(level)
    }
}
pub(crate) fn step(current: u8, delta: i8, max: u8) -> u8 {
    (i16::from(current) + i16::from(delta)).clamp(1, i16::from(max)) as u8
}
#[cfg(test)]
mod tests {
    #[test]
    fn keyboard_navigation_only_binds_the_original_known_type() {
        let mut profile = super::LightingProfile::default();
        assert!(super::navigation_identity(&profile).is_err());
        profile.detected_type = Some(6);
        assert_eq!(
            super::navigation_identity(&profile).unwrap(),
            "keyboard-panel:6"
        );
        profile.detected_type = Some(19);
        assert!(super::navigation_identity(&profile).is_err());
    }
    #[test]
    fn original_brightness_keys_enable_light_and_keep_nonzero_minimum() {
        for (current, delta, max, expected) in [
            (0, -1, 4, 1),
            (1, -1, 4, 1),
            (4, 1, 4, 4),
            (4, 1, 5, 5),
            (3, -1, 4, 2),
        ] {
            assert_eq!(super::step(current, delta, max), expected);
        }
    }
}
