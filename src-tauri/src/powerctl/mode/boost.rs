//! 性能选项属于原厂模式域，恢复与模式/档案共用同一基线。
use super::*;
use crate::fanctl::{
    self, FAN_MODE_AUTO, FAN_MODE_CUSTOM, FAN_MODE_NOISELESS, NOISELESS_FAN_LABEL,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
#[path = "boost/options.rs"]
mod options;
pub(crate) use options::{
    Change, FanTarget, Plan, ensure_fan_allowed, ensure_fan_allowed_on, validate_active_fan,
};
mod driver;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod workflow_tests;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Values {
    pub turbo_fan: Option<bool>,
    pub dtt: Option<bool>,
}
impl Values {
    pub fn validate(&self) -> Result<()> {
        if self.turbo_fan.is_none() && self.dtt.is_none() {
            bail!("性能选项档案没有目标");
        }
        Ok(())
    }
    fn preferences(&self, before: [u8; 2], caps: [bool; 2]) -> Result<[u8; 2]> {
        let mut result = before;
        for (index, value) in [self.turbo_fan, self.dtt].into_iter().enumerate() {
            if caps[index] != value.is_some() {
                bail!("所选性能选项与当前设备能力不符");
            }
            if let Some(value) = value {
                result[index] = u8::from(value);
            }
        }
        Ok(result)
    }

    fn select(&self, previous: Self) -> Self {
        Self {
            turbo_fan: self.turbo_fan.or(previous.turbo_fan),
            dtt: self.dtt.or(previous.dtt),
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum QuietTarget {
    Current,
    EntertainmentOn,
    EntertainmentOff,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub expected_revision: String,
    pub values: Values,
    #[serde(deserialize_with = "Option::deserialize")]
    pub quiet_fan: Option<QuietTarget>,
    pub replace_custom_fan: bool,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuietFan {
    pub enabled: bool,
    pub active: bool,
    pub remembered: bool,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub current: Values,
    pub quiet_fan: Option<QuietFan>,
    pub current_mode: u8,
    pub applies_in_current_mode: bool,
    pub turbo_active: bool,
    pub dtt_active: bool,
    pub dtt_driver_available: Option<bool>,
    pub revision: String,
    pub warnings: Vec<String>,
    pub blocked_reason: Option<String>,
}
struct Preview {
    core: CoreSnapshot,
    caps: [bool; 2],
    runtime_options: [bool; 2],
    fan: u8,
    quiet_fan: Option<QuietFan>,
    driver: Option<bool>,
    revision: String,
}
fn capabilities(io: &impl ModeIo) -> Result<[bool; 2]> {
    let caps = io.read(7, 0, 18).map_err(|e| anyhow!(e))?;
    if reviewed_option_command(io, &caps) {
        return Ok([true; 2]);
    }
    advertised_options(io, &caps)
}

fn reviewed_option_command(io: &impl ModeIo, caps: &[u8]) -> bool {
    // The reviewed SCMD selects the command path with PSF4 bit 2, independently of OEM UI flags.
    io.reviewed_entertainment_options()
        && caps.len() >= 18
        && caps.starts_with(&[1, 0])
        && caps.get(10).is_some_and(|value| value & 4 != 0)
}

fn advertised_options(io: &impl ModeIo, caps: &[u8]) -> Result<[bool; 2]> {
    if caps.len() != 18 {
        bail!("性能选项能力页长度不足");
    }
    match &caps[..2] {
        [0, 0] => Ok([false, super::feature_flags(io, caps, 96)? & (1 << 12) != 0]),
        [1, 0] => Ok([
            caps[16] & 16 != 0,
            super::feature_flags(io, caps, 96)? & (1 << 12) != 0,
        ]),
        _ => bail!("未知性能选项能力页版本"),
    }
}

pub(super) fn runtime_options(io: &impl ModeIo, mode: u8) -> Result<[bool; 2]> {
    match mode {
        2 => Ok([true, true]),
        3 if io.reviewed_entertainment_options() => {
            let caps = io.read(7, 0, 18).map_err(|e| anyhow!(e))?;
            Ok([reviewed_option_command(io, &caps); 2])
        }
        _ => Ok([false, false]),
    }
}

fn option_command(mode: u8, [turbo, dtt]: [u8; 2], [can_turbo, can_dtt]: [bool; 2]) -> u32 {
    mode_command(mode, [turbo * u8::from(can_turbo), dtt * u8::from(can_dtt)])
}

pub(super) fn command_for_current_preferences(
    io: &impl ModeIo,
    mode: u8,
    values: [u8; 2],
) -> Result<u32> {
    Ok(option_command(mode, values, runtime_options(io, mode)?))
}

pub(super) fn validate_entertainment_command(io: &impl ModeIo, command: u32) -> Result<()> {
    if command & 0x3f == 3 && command & 0xc0 != 0 {
        let allowed = command_for_current_preferences(io, 3, [1, 1])?;
        if command & !allowed != 0 {
            bail!("当前固件的娱乐档性能选项命令不可用");
        }
    }
    Ok(())
}
fn preview(io: &impl ModeIo, driver: Option<bool>) -> Result<Preview> {
    let caps = capabilities(io)?;
    let core = capture(io, false)?;
    let runtime_options = runtime_options(io, core.mode)?;
    let fan = ccc_core::fan::decode_oem_fan_mode(io.fan_info()?[14]).map_err(anyhow::Error::msg)?;
    let quiet_fan = quiet_fan(io, core.mode, fan, core.command & 0x40 != 0)?;
    let revision = format!(
        "{:x}",
        Sha256::digest(
            format!(
                "{caps:?}:{runtime_options:?}:{}:{}:{:?}:{fan}:{driver:?}:{quiet_fan:?}",
                core.mode, core.command, core.preferences
            )
            .as_bytes()
        )
    );
    Ok(Preview {
        core,
        caps,
        runtime_options,
        fan,
        quiet_fan,
        driver,
        revision,
    })
}

fn quiet_fan(io: &impl ModeIo, power_mode: u8, fan: u8, turbo: bool) -> Result<Option<QuietFan>> {
    if !matches!(power_mode, 2 | 3) {
        return Ok(None);
    }
    let caps = io.read(7, 0, 22).map_err(|e| anyhow!(e))?;
    if !fanctl::supports_noiseless(&caps) {
        return Ok(None);
    }
    if power_mode == 3 {
        let enabled = fanctl::saved::entertainment_quiet(io)?;
        return Ok(Some(QuietFan {
            enabled,
            active: !turbo && fan == FAN_MODE_NOISELESS,
            remembered: true,
        }));
    }
    let remembered = fanctl::saved::remembered_mode(io, &caps, power_mode)?;
    Ok(Some(QuietFan {
        enabled: remembered.unwrap_or(fan) == FAN_MODE_NOISELESS,
        active: fan == FAN_MODE_NOISELESS,
        remembered: remembered.is_some(),
    }))
}

fn turbo_enabled(io: &impl ModeIo, mode: u8) -> Result<bool> {
    if mode == 3 {
        return Ok(runtime_options(io, mode)?[0]
            && io
                .acknowledged_entertainment_command()
                .is_some_and(|command| command & 0x40 != 0));
    }
    Ok(mode == 2 && byte(io, 4, 8)? != 0)
}

pub(super) fn prepare_recovery(
    hal: &Hal,
    target: CoreSnapshot,
    fan_attempted: &mut bool,
) -> Result<()> {
    if target.mode == 3 && target.command & 0xc0 != 0 {
        hal.with_dchu(|io| validate_entertainment_command(io, target.command))?;
        if target.command & 0x80 != 0 {
            require_driver(hal)?;
        }
    }
    if target.command & 0x40 != 0 {
        *fan_attempted |= fanctl::software::active(hal)?;
        fanctl::software::stop_safely(hal)?;
        fanctl::handoff_for_turbo(hal, fan_attempted)?;
    }
    Ok(())
}

impl Preview {
    fn values(&self) -> Values {
        self.values_for(self.core.preferences)
    }
    fn values_for(&self, preferences: [u8; 2]) -> Values {
        Values {
            turbo_fan: self.caps[0].then_some(preferences[0] == 1),
            dtt: self.caps[1].then_some(preferences[1] == 1),
        }
    }
    fn target(&self, request: &Request) -> Result<(CoreSnapshot, Option<u8>)> {
        if request.expected_revision != self.revision {
            bail!("性能选项、模式、风扇或驱动状态已变化，请重新预览");
        }
        if !matches!(self.core.mode, 2 | 3) {
            bail!("请在性能模式或娱乐模式配置性能选项");
        }
        if request.quiet_fan.is_none() {
            request.values.validate()?;
        }
        let preferences = request
            .values
            .select(self.values())
            .preferences(self.core.preferences, self.caps)?;
        let command = option_command(
            self.core.mode,
            [
                u8::from(
                    request
                        .values
                        .turbo_fan
                        .unwrap_or(self.core.command & 0x40 != 0),
                ),
                u8::from(request.values.dtt.unwrap_or(self.core.command & 0x80 != 0)),
            ],
            self.runtime_options,
        );
        let fan = self.fan_target(command, request.quiet_fan)?;
        if request.replace_custom_fan {
            let mut expected = self.core.preferences;
            let turbo_handoff = self.core.mode == 3
                && self.runtime_options[0]
                && fan == Some(FAN_MODE_AUTO)
                && self.core.command & 0x40 == 0
                && preferences[0] == 1;
            if turbo_handoff {
                expected[0] = 1;
            } else if self.core.command & 0x40 != 0 && fan == Some(FAN_MODE_NOISELESS) {
                expected[0] = 0;
            }
            if self.fan != FAN_MODE_CUSTOM
                || fan.is_none()
                || !turbo_handoff && request.quiet_fan.is_none()
                || preferences != expected
            {
                bail!("自定义风扇切换目标已变化，请重新确认");
            }
        }
        if fan.is_some() && self.fan == FAN_MODE_CUSTOM && !request.replace_custom_fan {
            bail!("请先确认切换自定义固件风扇曲线");
        }
        if self.runtime_options[1]
            && command & 0x80 != 0
            && (preferences != self.core.preferences || command != self.core.command)
            && self.driver != Some(true)
        {
            bail!("未确认Dynamic Tuning驱动，不能启用DTT");
        }
        let core = CoreSnapshot {
            preferences,
            command,
            ..self.core
        };
        Ok((core, fan))
    }
    fn fan_target(&self, command: u32, wanted: Option<QuietTarget>) -> Result<Option<u8>> {
        let turbo = command & 0x40 != 0;
        let turbo_changed = (command ^ self.core.command) & 0x40 != 0;
        if let Some(wanted) = wanted {
            let current = self
                .quiet_fan
                .as_ref()
                .ok_or_else(|| anyhow!("当前模式或固件未开放{NOISELESS_FAN_LABEL}"))?;
            if wanted != QuietTarget::Current && self.core.mode != 3 {
                bail!("仅娱乐模式可设置娱乐档寂静偏好");
            }
            let enabled = wanted != QuietTarget::EntertainmentOff;
            if enabled && turbo {
                bail!("Turbo 风扇与{NOISELESS_FAN_LABEL}不能同时开启");
            }
            let preference_changed = wanted != QuietTarget::Current && enabled != current.enabled;
            if preference_changed || turbo_changed || (enabled && !current.active) {
                return Ok(Some(if enabled {
                    FAN_MODE_NOISELESS
                } else {
                    FAN_MODE_AUTO
                }));
            }
        }
        Ok(turbo_changed.then_some(FAN_MODE_AUTO))
    }
}
pub(super) fn write_target(io: &impl ModeIo, target: CoreSnapshot) -> Result<()> {
    validate_entertainment_command(io, target.command)?;
    write_saved(io, target.preferences)?;
    if runtime_options(io, target.mode)? != [false; 2] {
        io.send_mode(target.command)?;
    }
    let after = capture(io, false)?;
    if after.mode != target.mode || after.preferences != target.preferences {
        bail!("性能选项保存值回读不一致");
    }
    Ok(())
}
pub(super) fn write_saved(io: &impl ModeIo, preferences: [u8; 2]) -> Result<()> {
    for (page, offset, value) in [(4, 8, preferences[0]), (1, 32, preferences[1])] {
        if byte(io, page, offset)? != value {
            settings::write_verified(io, page, &[Patch::new(offset, [value])], None)
                .map_err(|error| anyhow!(error))?;
        }
    }
    Ok(())
}
fn installed(hal: &Hal, reviewed: bool) -> Result<bool> {
    let _ = (hal, reviewed);
    #[cfg(test)]
    return (hal
        .test_dtt_driver
        .as_ref()
        .ok_or_else(|| anyhow!("隔离HAL未注入DTT驱动查询"))?)();
    #[cfg(not(test))]
    {
        if reviewed {
            driver::active_platform()
        } else {
            driver::installed()
        }
    }
}
fn driver_status(hal: &Hal) -> Result<(Option<bool>, Vec<String>)> {
    let check = hal.with_dchu(|io| {
        let caps = io.read(7, 0, 18).map_err(|e| anyhow!(e))?;
        let reviewed = reviewed_option_command(io, &caps);
        Ok((reviewed || advertised_options(io, &caps)?[1]).then_some(reviewed))
    })?;
    let Some(reviewed) = check else {
        return Ok((None, vec![]));
    };
    Ok(match installed(hal, reviewed) {
        Ok(found) => (Some(found), vec![]),
        Err(error) => (None, vec![format!("DTT驱动核对失败：{error:#}")]),
    })
}
pub(super) fn require_driver(hal: &Hal) -> Result<()> {
    let (available, warnings) = driver_status(hal)?;
    if available != Some(true) {
        bail!(
            "未确认Dynamic Tuning驱动，不能激活DTT{}",
            if warnings.is_empty() {
                String::new()
            } else {
                format!("：{}", warnings.join("；"))
            }
        );
    }
    Ok(())
}
pub(super) fn revision(hal: &Hal) -> Result<String> {
    let (driver, _) = driver_status(hal)?;
    hal.with_dchu(|io| preview(io, driver).map(|current| current.revision))
}
pub(crate) fn capture_values(hal: &Hal) -> Result<Option<Values>> {
    hal.with_dchu(|io| {
        if super::support::issue(io)?.is_some() {
            return Ok(None);
        }
        let caps = capabilities(io)?;
        if caps == [false; 2] {
            return Ok(None);
        }
        let current = capture(io, false)?;
        Ok(Some(Values {
            turbo_fan: caps[0].then_some(current.preferences[0] == 1),
            dtt: caps[1].then_some(current.preferences[1] == 1),
        }))
    })
}
pub(super) fn profile_preferences(
    io: &impl ModeIo,
    before: [u8; 2],
    values: Option<&Values>,
) -> Result<[u8; 2]> {
    match values {
        Some(values) => values.preferences(before, capabilities(io)?),
        None => Ok(before),
    }
}
pub fn read(hal: &Hal) -> Result<Profile> {
    let support_issue = hal.with_dchu(|io| super::support::issue(io))?;
    let (driver, warnings) = driver_status(hal)?;
    let p = hal.with_dchu(|io| preview(io, driver))?;
    Ok(Profile {
        current: p.values(),
        quiet_fan: p.quiet_fan,
        current_mode: p.core.mode,
        applies_in_current_mode: p.runtime_options != [false; 2],
        turbo_active: p.core.command & 0x40 != 0,
        dtt_active: p.core.command & 0x80 != 0,
        dtt_driver_available: driver,
        revision: p.revision,
        warnings,
        blocked_reason: support_issue.or_else(|| {
            hal.ensure_mode_change_allowed()
                .and_then(|_| fanctl::ensure_manual_allowed(hal))
                .err()
                .map(|e| format!("{e:#}"))
        }),
    })
}
pub fn apply(hal: &Hal, request: &Request) -> Result<()> {
    hal.with_dchu(|io| super::support::ensure(io))?;
    let (driver, _) = driver_status(hal)?;
    let current = hal.with_dchu(|io| preview(io, driver))?;
    let (next, fan) = current.target(request)?;
    if (current.runtime_options != [false; 2] || fan.is_some()) && fanctl::software::active(hal)? {
        bail!("请先退出软件风扇曲线，再应用性能选项");
    }
    let remember_quiet = request.quiet_fan != Some(QuietTarget::Current);
    let plan = Plan::from_preview(hal, current, next, fan, remember_quiet)?;
    plan.execute(hal, false, &mut Change::default())
}
