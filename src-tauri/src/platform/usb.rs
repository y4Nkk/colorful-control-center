//! 原厂USB 选择性暂停：固件建议与Windows三个标准计划独立表达。
use crate::hal::{
    Hal,
    power_plan::{PowerAccess, SCHEMES},
};
use anyhow::{Context, Result, anyhow, bail};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Mutex,
};
#[path = "usb/model.rs"]
mod model;
#[path = "usb/scope.rs"]
mod scope;
#[cfg(test)]
#[path = "usb/tests.rs"]
mod tests;
#[path = "usb/transaction.rs"]
mod transaction;
pub(crate) use model::Change;
use model::{Baseline, Frame, PLANS, PlanState};
pub use model::{Profile, Request, Target};
pub(crate) use scope::{Plan, Scope};

pub(crate) trait Io {
    fn firmware(&self) -> Result<Option<u8>>;
    fn power(&self) -> &dyn PowerAccess;
    fn ensure_write(&self) -> Result<()>;
}
fn read_firmware(io: &(impl Io + ?Sized)) -> Result<Option<u8>> {
    let firmware = io.firmware()?;
    if firmware.is_some_and(|v| v > 2) {
        bail!("固件USB建议未知");
    }
    Ok(firmware)
}
fn read_frame(power: &dyn PowerAccess, firmware: Option<u8>) -> Result<Frame> {
    let schemes = power.schemes()?;
    if schemes.iter().collect::<BTreeSet<_>>().len() != schemes.len() {
        bail!("电源计划枚举重复");
    }
    let active = power.active()?;
    if !schemes.contains(&active) {
        bail!("活动计划与已枚举计划不一致，请重新读取");
    }
    let plans = PLANS
        .into_iter()
        .filter(|id| schemes.contains(&id.guid()))
        .map(|id| power.usb(id.guid()).map(|values| (id, values)))
        .collect::<Result<_>>()?;
    Ok(Frame {
        firmware,
        active,
        plans,
    })
}
fn capture(io: &(impl Io + ?Sized), recovering: bool) -> Result<Option<Frame>> {
    let firmware = read_firmware(io)?;
    if firmware.is_none() && !recovering {
        return Ok(None);
    }
    read_frame(io.power(), firmware).map(Some)
}
fn read_active_values(
    power: &dyn PowerAccess,
    frame: &Frame,
    plan: Option<model::PlanId>,
) -> Result<model::Targets> {
    let [ac, dc] = plan
        .and_then(|id| frame.plans.get(&id).copied())
        .map_or_else(|| power.usb(frame.active), Ok)?;
    Ok(model::Targets {
        ac: model::flag(ac),
        dc: model::flag(dc),
    })
}
fn profile(io: &(impl Io + ?Sized), baseline: &Baseline) -> Result<Profile> {
    let (firmware, firmware_error) = match read_firmware(io) {
        Ok(value) => (value, None),
        Err(error) => (None, Some(format!("固件 USB 建议读取失败：{error:#}"))),
    };
    let power = io.power();
    let frame = read_frame(power, firmware)?;
    let blocked = firmware_error.or_else(|| {
        if firmware.is_none() && baseline.is_empty() {
            return None;
        }
        io.ensure_write().err().map(|e| format!("{e:#}"))
    });
    let mut warnings = Vec::new();
    let plans = frame
        .plans
        .iter()
        .map(|(id, values)| {
            if values.iter().any(|v| *v > 1) {
                warnings.push(format!("{id:?}包含未知USB设置，未修改字段可保持现值。"));
            }
            PlanState {
                id: *id,
                ac: model::flag(values[0]),
                dc: model::flag(values[1]),
            }
        })
        .collect();
    let active_plan = PLANS.into_iter().find(|id| id.guid() == frame.active);
    let active_values = read_active_values(power, &frame, active_plan).unwrap_or_else(|error| {
        warnings.push(format!("活动计划 USB 设置读取失败：{error:#}"));
        model::Targets::default()
    });
    let capability_block = firmware.is_none().then(|| {
        if baseline.is_empty() {
            "固件未声明 USB 选择性暂停，Windows 设置为只读。".into()
        } else {
            "固件未声明 USB 选择性暂停，仅保留初始恢复。".into()
        }
    });
    let no_plans = frame.plans.is_empty().then_some("没有可接管的标准计划。");
    Ok(Profile {
        revision: frame.revision()?,
        recommendation: frame.firmware.and_then(|v| match v {
            1 => Some(true),
            2 => Some(false),
            _ => None,
        }),
        supported: frame.firmware.is_some(),
        active_plan,
        active_values,
        plans,
        missing: PLANS
            .into_iter()
            .filter(|id| !frame.plans.contains_key(id))
            .collect(),
        initial: baseline
            .iter()
            .map(|(id, values)| PlanState {
                id: *id,
                ac: values[0].and_then(model::flag),
                dc: values[1].and_then(model::flag),
            })
            .collect(),
        restore_blocked_reason: blocked.clone().or_else(|| {
            if baseline.is_empty() {
                None
            } else {
                frame
                    .initial_changes(baseline)
                    .err()
                    .map(|e| format!("{e:#}"))
            }
        }),
        blocked_reason: blocked
            .or(capability_block)
            .or_else(|| no_plans.map(String::from)),
        warnings,
    })
}
#[cfg(not(test))]
struct Native<'a>(&'a Hal);
#[cfg(not(test))]
impl Io for Native<'_> {
    fn firmware(&self) -> Result<Option<u8>> {
        self.0.with_dchu(|d| {
            if !model::supported(&d.read_bios_capabilities(0, 24)?)? {
                return Ok(None);
            }
            let mut packet = [0; 256];
            packet[0] = 53;
            Ok(Some(d.exchange_package(4, &packet)?[0]))
        })
    }
    fn power(&self) -> &dyn PowerAccess {
        self.0.system_power.as_ref()
    }
    fn ensure_write(&self) -> Result<()> {
        self.0
            .ensure_mode_change_allowed()
            .context("电池校准期间不能修改USB系统策略")?;
        crate::hal::oem_owner::ensure_owner_absent()
    }
}
#[derive(Default)]
pub struct Controller {
    baseline: Mutex<Baseline>,
    #[cfg(test)]
    io: Option<std::sync::Arc<dyn Io + Send + Sync>>,
}
impl Controller {
    pub fn read(&self, hal: &Hal) -> Result<Profile> {
        let scope = self.scope()?;
        let _gate = hal.begin_write()?;
        scope.with_io(hal, |io| profile(io, &scope.baseline))
    }
    pub fn apply(&self, hal: &Hal, request: &Request) -> Result<()> {
        let mut scope = self.scope()?;
        let _gate = hal.begin_write()?;
        let baseline = &mut scope.baseline;
        scope.controller.with_io(hal, |io| {
            let frame = capture(io, !baseline.is_empty())?.context("固件未声明 USB 选择性暂停")?;
            transaction::apply(io, baseline, frame, request)
        })
    }
    pub fn restore(&self, hal: &Hal, expected: &str) -> Result<()> {
        let scope = self.scope()?;
        let _gate = hal.begin_write()?;
        scope.with_io(hal, |io| {
            let frame = capture(io, true)?.context("USB恢复状态不可用")?;
            transaction::restore(io, &scope.baseline, frame, expected)
        })
    }
}
