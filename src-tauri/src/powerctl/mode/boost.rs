//! 性能选项属于原厂模式域，恢复与模式/档案共用同一基线。
use super::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
#[path = "boost/options.rs"]
mod options;
pub(crate) use options::{Change, FanTarget, Plan, validate_fan, validate_profile};
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
        self.validate()?;
        let mut result = before;
        for (index, value) in [self.turbo_fan, self.dtt].into_iter().enumerate() {
            if caps[index] != value.is_some() {
                bail!("只能修改固件声明支持的性能选项");
            }
            if let Some(value) = value {
                result[index] = u8::from(value);
            }
        }
        Ok(result)
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub expected_revision: String,
    pub values: Values,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub current: Values,
    pub current_mode: u8,
    pub ac_mode: Option<u8>,
    pub energy: Option<energy::State>,
    pub restore: Option<super::workflow::RestoreTarget>,
    pub restore_blocked_reason: Option<String>,
    pub custom_fan: bool,
    pub software_active: bool,
    pub dtt_driver_available: Option<bool>,
    pub revision: String,
    pub can_restore: bool,
    pub warnings: Vec<String>,
    pub blocked_reason: Option<String>,
}
struct Preview {
    core: CoreSnapshot,
    caps: [bool; 2],
    fan: u8,
    driver: Option<bool>,
    revision: String,
}
fn capabilities(io: &impl ModeIo) -> Result<[bool; 2]> {
    let caps = io.read(7, 0, 18).map_err(|e| anyhow!(e))?;
    if caps.len() != 18 {
        bail!("性能选项能力页长度不足");
    }
    match &caps[..2] {
        [0, 0] => Ok([false, super::feature_flags(io, &caps, 96)? & (1 << 12) != 0]),
        [1, 0] => Ok([
            caps[16] & 16 != 0,
            super::feature_flags(io, &caps, 96)? & (1 << 12) != 0,
        ]),
        _ => bail!("未知性能选项能力页版本"),
    }
}
fn preview(io: &impl ModeIo, driver: Option<bool>) -> Result<Preview> {
    let caps = capabilities(io)?;
    let core = capture(io, false)?;
    let fan = byte(io, 4, 5)?;
    crate::fanctl::validate_mode(fan)?;
    let revision = format!(
        "{:x}",
        Sha256::digest(
            format!(
                "{:?}:{}:{:?}:{fan}:{driver:?}",
                caps, core.mode, core.preferences
            )
            .as_bytes()
        )
    );
    Ok(Preview {
        core,
        caps,
        fan,
        driver,
        revision,
    })
}
impl Preview {
    fn values(&self) -> Values {
        Values {
            turbo_fan: self.caps[0].then_some(self.core.preferences[0] == 1),
            dtt: self.caps[1].then_some(self.core.preferences[1] == 1),
        }
    }
    fn target(&self, request: &Request) -> Result<CoreSnapshot> {
        if request.expected_revision != self.revision {
            bail!("性能选项、模式、风扇或驱动状态已变化，请重新预览");
        }
        if self.core.mode != 2 {
            bail!("请先切换到性能模式，再应用性能选项");
        }
        let preferences = request
            .values
            .preferences(self.core.preferences, self.caps)?;
        if preferences[0] != self.core.preferences[0] && self.fan == 6 {
            bail!("请先退出自定义固件风扇曲线，再修改Turbo风扇");
        }
        if preferences[1] == 1 && preferences != self.core.preferences && self.driver != Some(true)
        {
            bail!("未确认Dynamic Tuning驱动，不能启用DTT");
        }
        Ok(CoreSnapshot {
            preferences,
            command: mode_command(2, preferences),
            ..self.core
        })
    }
}
pub(super) fn write_target(io: &impl ModeIo, target: CoreSnapshot) -> Result<()> {
    write_saved(io, target.preferences)?;
    if target.mode == 2 {
        io.send_mode(target.command)?;
    }
    let after = capture(io, false)?;
    if after.mode != target.mode || after.preferences != target.preferences {
        bail!("性能选项保存值回读不一致");
    }
    Ok(())
}
pub(super) fn write_saved(io: &impl ModeIo, preferences: [u8; 2]) -> Result<()> {
    settings::apply(io, 4, &[Patch::new(8, [preferences[0]])], None).map_err(|e| anyhow!(e))?;
    settings::apply(io, 1, &[Patch::new(32, [preferences[1]])], None).map_err(|e| anyhow!(e))?;
    Ok(())
}
fn installed(hal: &Hal) -> Result<bool> {
    #[cfg(test)]
    return (hal
        .test_dtt_driver
        .as_ref()
        .ok_or_else(|| anyhow!("隔离HAL未注入DTT驱动查询"))?)();
    #[cfg(not(test))]
    {
        let _ = hal;
        driver::installed()
    }
}
fn driver_status(hal: &Hal) -> Result<(Option<bool>, Vec<String>)> {
    if !hal.with_dchu(|io| capabilities(io).map(|caps| caps[1]))? {
        return Ok((None, vec![]));
    }
    Ok(match installed(hal) {
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
pub(super) fn configure(
    io: &impl ModeIo,
    before: CoreSnapshot,
    target: &mut Target,
    values: Option<&Values>,
) -> Result<[u8; 2]> {
    let Some(values) = values else {
        return Ok(before.preferences);
    };
    let preferences = values.preferences(before.preferences, capabilities(io)?)?;
    target.turbo = target.mode == 2 && preferences[0] == 1;
    target.dtt = target.mode == 2 && preferences[1] == 1;
    if target.mode == 2 && preferences[0] != before.preferences[0] {
        target.fan_mode = 0;
        target.fan_saved_mode = 0;
    }
    Ok(preferences)
}
pub fn read(hal: &Hal) -> Result<Profile> {
    let support_issue = hal.with_dchu(|io| super::support::issue(io))?;
    let (driver, warnings) = driver_status(hal)?;
    let p = hal.with_dchu(|io| preview(io, driver))?;
    let restore = super::workflow::restore_target(hal)?;
    let energy = if restore
        .as_ref()
        .is_some_and(|target| target.energy.is_some())
    {
        Some(hal.with_dchu(|io| energy::read(io, hal.reviewed_dsdt))?)
    } else {
        None
    };
    let ac_mode = if restore
        .as_ref()
        .is_some_and(|target| target.ac_mode.is_some())
    {
        Some(hal.with_dchu(|io| byte(io, 1, 16))?)
    } else {
        None
    };
    let restore_blocked_reason =
        energy::restore_issue(restore.as_ref().and_then(|target| target.energy), energy);
    Ok(Profile {
        current: p.values(),
        current_mode: p.core.mode,
        ac_mode,
        energy,
        custom_fan: p.fan == 6,
        software_active: crate::fanctl::software::read(hal)?.is_some(),
        dtt_driver_available: driver,
        revision: p.revision,
        can_restore: restore.is_some() && restore_blocked_reason.is_none(),
        restore_blocked_reason,
        restore,
        warnings,
        blocked_reason: support_issue.or_else(|| {
            hal.ensure_mode_change_allowed()
                .and_then(|_| crate::fanctl::dust::ensure_manual_allowed(hal))
                .err()
                .map(|e| format!("{e:#}"))
        }),
    })
}
pub fn apply(hal: &Hal, request: &Request) -> Result<()> {
    hal.with_dchu(|io| super::support::ensure(io))?;
    if crate::fanctl::software::read(hal)?.is_some() {
        bail!("请先退出软件风扇曲线，再应用性能选项");
    }
    let (driver, _) = driver_status(hal)?;
    let current = hal.with_dchu(|io| preview(io, driver))?;
    let next = current.target(request)?;
    if next.preferences == current.core.preferences {
        return Ok(());
    }
    let plan = Plan::prepare(hal, &request.values)?;
    if plan.revision != request.expected_revision {
        bail!("性能选项已变化，请重新预览");
    }
    plan.execute(hal, false, &mut Change::default())
}
