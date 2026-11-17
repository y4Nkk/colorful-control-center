use super::*;
use anyhow::{Context, bail};

pub(crate) fn identity(profile: &LightingProfile) -> Result<String> {
    if profile.detected_type != Some(2) || profile.detected_family != KeyboardFamily::ZoneRgb {
        bail!("此键盘未提供原厂分区开关");
    }
    hotkey::identity(profile)
}
impl LightingController {
    pub(crate) fn toggle_zone(&self, hal: &Hal, expected: &str, zone: u8) -> Result<bool> {
        if zone > 2 {
            bail!("键盘分区必须为左、中或右");
        }
        let mut scope = self.scope()?;
        let _gate = hal.begin_write()?;
        let profile = hal.with_dchu(|d| Ok(build_profile(d, None)))?;
        if identity(&profile)? != expected {
            bail!("键盘类型已变化，请重新确认分区热键");
        }
        let mut target = scope.capture(hal)?.context("键盘灯保存状态未知")?;
        let LightingRequest::ZoneRgb { zones, .. } = &mut target.settings else {
            bail!("当前键盘不是原厂三区键盘");
        };
        let selected = &mut zones[usize::from(zone)];
        selected.enabled = !selected.enabled;
        let enabled = selected.enabled;
        let plan = scope.prepare(hal, &target)?;
        scope.execute(hal, &plan, &mut false)?;
        Ok(enabled)
    }
}
