use crate::hal::{Hal, system_action::Kind};
use anyhow::{Result, bail};
use serde::Serialize;
use std::time::Duration;
mod native;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Snapshot {
    pub plan: String,
    pub actions: [Option<Kind>; 2],
    pub modern_standby: bool,
}
impl Snapshot {
    fn new(plan: u128, raw: [u32; 2], modern_standby: bool) -> Result<Self> {
        let decode = |value| match value {
            0 => Ok(None),
            1 if modern_standby => Ok(Some(Kind::ScreenOff)),
            1 => Ok(Some(Kind::Sleep)),
            2 => Ok(Some(Kind::Hibernate)),
            3 => Ok(Some(Kind::Shutdown)),
            _ => bail!("Windows合盖动作编号未知"),
        };
        Ok(Self {
            plan: format!("{plan:032x}"),
            actions: [decode(raw[0])?, decode(raw[1])?],
            modern_standby,
        })
    }
    fn identity(&self) -> Result<String> {
        super::model::digest(&("oem-lid-action-1", self))
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Profile {
    pub identity: Option<String>,
    pub current: Option<Snapshot>,
    pub on_ac: Option<bool>,
    pub power_error: Option<String>,
    pub error: Option<String>,
}

trait Io {
    fn locked<T>(&self, f: impl FnOnce() -> Result<T>) -> Result<T>;
    fn snapshot(&self) -> Result<Snapshot>;
    fn on_ac(&self) -> Result<bool>;
    fn ensure_writable(&self) -> Result<()>;
    fn request(&self, kind: Kind) -> Result<()>;
}
fn target(io: &impl Io, event: u8, expected: &str) -> Result<(Option<Kind>, Option<bool>)> {
    io.ensure_writable()?;
    match event {
        72 if expected == SHUTDOWN_IDENTITY => Ok((Some(Kind::Shutdown), None)),
        192 => {
            let snapshot = io.snapshot()?;
            if snapshot.identity()? != expected {
                bail!("合盖计划或待机能力已变化，请重新确认");
            }
            let on_ac = io.on_ac()?;
            Ok((snapshot.actions[usize::from(!on_ac)], Some(on_ac)))
        }
        _ => bail!("系统电源事件或授权无效"),
    }
}
fn run(
    io: &impl Io,
    event: u8,
    expected: &str,
    mut permit: impl FnMut() -> Result<()>,
    mut wait: impl FnMut(Duration) -> Result<bool>,
) -> Result<Option<String>> {
    let before = io.locked(|| {
        permit()?;
        target(io, event, expected)
    })?;
    let Some(kind) = before.0 else {
        return Ok(Some("当前合盖设置为不执行操作".into()));
    };
    if kind == Kind::ScreenOff && !wait(Duration::from_millis(200))? {
        return Ok(None);
    }
    io.locked(|| {
        permit()?;
        if target(io, event, expected)? != before {
            bail!("供电或合盖目标在请求前发生变化");
        }
        Ok(())
    })?;
    permit()?;
    io.request(kind)?;
    Ok(Some(format!("已请求系统{}", kind.label())))
}
pub(super) const SHUTDOWN_IDENTITY: &str = "oem-shutdown-event-1";
pub(super) fn shutdown_identity(hal: &Hal) -> Result<String> {
    if !hal.reviewed_dsdt {
        bail!("当前机型的原厂系统电源事件尚未核对");
    }
    Ok(SHUTDOWN_IDENTITY.into())
}
pub(super) fn profile(hal: &Hal) -> Profile {
    native::profile(hal)
}
pub(super) fn execute(
    hal: &Hal,
    event: u8,
    expected: &str,
    permit: impl FnMut() -> Result<()>,
    wait: impl FnMut(Duration) -> Result<bool>,
) -> Result<Option<String>> {
    run(&native::Native(hal), event, expected, permit, wait)
}
#[cfg(test)]
mod tests;
