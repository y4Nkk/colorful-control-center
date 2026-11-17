use crate::{
    battery::PowerStatus,
    hal::{Hal, power_plan::BatteryAction},
};
use anyhow::{Result, bail};
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub plan: String,
    pub low: u32,
    pub critical: u32,
}
impl Snapshot {
    fn identity(&self) -> Result<String> {
        super::model::digest(&("battery-low-action-1", self))
    }
    fn action(&self, event: u8) -> Result<u32> {
        match event {
            44 => Ok(self.low),
            45 => Ok(self.critical),
            _ => bail!("未知低电量系统动作事件"),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub identity: Option<String>,
    pub current: Option<Snapshot>,
    pub on_ac: Option<bool>,
    pub percent: Option<u8>,
    pub error: Option<String>,
    pub power_error: Option<String>,
}

trait Io {
    fn locked<T>(&self, f: impl FnOnce() -> Result<T>) -> Result<T>;
    fn known_firmware(&self) -> bool;
    fn active(&self) -> Result<u128>;
    fn action(&self, plan: u128, action: BatteryAction) -> Result<u32>;
    fn power(&self) -> Result<PowerStatus>;
    fn signal(&self, level: u8) -> Result<()>;
    fn ensure_writable(&self) -> Result<()>;
    fn suspend(&self, hibernate: bool) -> Result<()>;
}
fn snapshot(io: &impl Io) -> Result<Snapshot> {
    if !io.known_firmware() {
        bail!("当前固件未经低电量轮询命令静态核对");
    }
    let plan = io.active()?;
    let low = io.action(plan, BatteryAction::Low)?;
    let critical = io.action(plan, BatteryAction::Critical)?;
    if low > 3 || critical > 3 {
        bail!("系统电池计划动作值未知");
    }
    if io.active()? != plan {
        bail!("读取电池计划动作期间活动计划已变化");
    }
    Ok(Snapshot {
        plan: format!("{plan:032x}"),
        low,
        critical,
    })
}
fn read(io: &impl Io) -> Profile {
    let result = io.locked(|| snapshot(io));
    let power = io.power();
    Profile {
        identity: result.as_ref().ok().and_then(|value| value.identity().ok()),
        current: result.as_ref().ok().cloned(),
        on_ac: power.as_ref().ok().and_then(|value| match value.ac_online {
            Some(false) if value.present != Some(true) => None,
            state => state,
        }),
        percent: power
            .as_ref()
            .ok()
            .and_then(|value| value.percent.filter(|percent| *percent <= 100)),
        error: result.err().map(|error| format!("{error:#}")),
        power_error: power.err().map(|error| format!("{error:#}")),
    }
}
fn confirmed(io: &impl Io, expected: &str, event: u8) -> Result<u32> {
    let current = snapshot(io)?;
    if current.identity()? != expected {
        bail!("电池计划动作或活动计划已变化，请重新确认");
    }
    current.action(event)
}
fn prepare(
    io: &impl Io,
    expected: &str,
    event: u8,
    permit: &mut impl FnMut() -> Result<()>,
) -> Result<()> {
    permit()?;
    io.ensure_writable()?;
    confirmed(io, expected, event)?;
    permit()?;
    io.signal(event - 43)
}
fn decide(
    io: &impl Io,
    expected: &str,
    event: u8,
    permit: &mut impl FnMut() -> Result<()>,
) -> Result<Option<u32>> {
    permit()?;
    io.ensure_writable()?;
    let action = confirmed(io, expected, event)?;
    let power = io.power()?;
    if power.ac_online == Some(true) {
        return Ok(None);
    }
    if power.ac_online != Some(false) || power.present != Some(true) {
        bail!("未确认电池供电，未请求系统睡眠或休眠");
    }
    permit()?;
    Ok(Some(action))
}
fn run(
    io: &impl Io,
    expected: &str,
    event: u8,
    mut permit: impl FnMut() -> Result<()>,
    mut wait: impl FnMut() -> Result<bool>,
) -> Result<Option<String>> {
    io.locked(|| prepare(io, expected, event, &mut permit))?;
    if !wait()? {
        return Ok(None);
    }
    let action = io.locked(|| decide(io, expected, event, &mut permit))?;
    let Some(action) = action else {
        return Ok(Some("已接电，未请求睡眠或休眠".into()));
    };
    match action {
        0 | 3 => Ok(Some("当前计划动作在原厂分支中不执行系统操作".into())),
        1 | 2 => {
            permit()?;
            io.ensure_writable()?;
            match io.locked(|| decide(io, expected, event, &mut permit))? {
                None => return Ok(Some("已接电，未请求睡眠或休眠".into())),
                Some(current) if current == action => (),
                Some(_) => bail!("电池计划动作变化，未请求系统睡眠或休眠"),
            }
            io.suspend(action == 2)?;
            Ok(Some(format!(
                "已向系统请求{}；实际状态以系统为准",
                if action == 1 { "睡眠" } else { "休眠" }
            )))
        }
        _ => bail!("未知电池计划动作"),
    }
}

struct Native<'a>(&'a Hal);
impl Io for Native<'_> {
    fn locked<T>(&self, f: impl FnOnce() -> Result<T>) -> Result<T> {
        let _gate = self.0.begin_write()?;
        f()
    }
    fn known_firmware(&self) -> bool {
        self.0.reviewed_dsdt
    }
    fn active(&self) -> Result<u128> {
        self.0.system_power.active()
    }
    fn action(&self, plan: u128, action: BatteryAction) -> Result<u32> {
        self.0.system_power.battery_action(plan, action)
    }
    fn power(&self) -> Result<PowerStatus> {
        crate::battery::system_power()
    }
    fn signal(&self, level: u8) -> Result<()> {
        if !matches!(level, 1 | 2) {
            bail!("低电量轮询通知参数无效");
        }
        let mut packet = [0; 256];
        packet[0] = 46;
        packet[1] = level;
        self.0
            .with_dchu(|io| io.exchange_package(4, &packet).map(|_| ()))
    }
    fn ensure_writable(&self) -> Result<()> {
        self.0.ensure_mode_change_allowed()?;
        #[cfg(test)]
        {
            bail!("隔离测试未注入系统动作许可");
        }
        #[cfg(not(test))]
        {
            crate::hal::oem_owner::ensure_owner_absent()?;
            crate::battery::calibration::ownership::ensure_available()
        }
    }
    fn suspend(&self, hibernate: bool) -> Result<()> {
        #[cfg(test)]
        {
            let _ = hibernate;
            bail!("隔离测试不调用真实系统睡眠");
        }
        #[cfg(not(test))]
        {
            crate::hal::system_action::request(if hibernate {
                crate::hal::system_action::Kind::Hibernate
            } else {
                crate::hal::system_action::Kind::Sleep
            })
        }
    }
}
pub(super) fn profile(hal: &Hal) -> Profile {
    read(&Native(hal))
}
pub(super) fn execute(
    hal: &Hal,
    expected: &str,
    event: u8,
    permit: impl FnMut() -> Result<()>,
    wait: impl FnMut() -> Result<bool>,
) -> Result<Option<String>> {
    run(&Native(hal), expected, event, permit, wait)
}

#[cfg(test)]
#[path = "low_battery_action/native_tests.rs"]
mod native_tests;
#[cfg(test)]
#[path = "low_battery_action/tests.rs"]
mod tests;
