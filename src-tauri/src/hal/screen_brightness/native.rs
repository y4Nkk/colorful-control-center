use super::Reading;
use crate::hal::wmi::Session;
use anyhow::{Context, Result, bail};
use windows::{
    Win32::System::{
        Variant::{VARIANT, VT_BOOL, VT_BSTR, VT_I4, VT_UI1, VT_UI4},
        Wmi::{IWbemClassObject, WBEM_S_FALSE, WBEM_S_TIMEDOUT},
    },
    core::{BSTR, PCWSTR, w},
};
use windows_sys::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};

fn property(object: &IWbemClassObject, name: PCWSTR) -> Result<VARIANT> {
    let mut value = VARIANT::default();
    unsafe {
        object.Get(name, 0, &mut value, None, None)?;
    }
    Ok(value)
}
fn parse(object: &IWbemClassObject, on_ac: bool) -> Result<Reading> {
    let active = property(object, w!("Active"))?;
    let panel = property(object, w!("InstanceName"))?;
    let level = property(object, w!("CurrentBrightness"))?;
    if active.vt() != VT_BOOL || !bool::try_from(&active)? || panel.vt() != VT_BSTR {
        bail!("屏幕活动状态或实例身份无效");
    }
    if !matches!(level.vt(), VT_UI1 | VT_I4 | VT_UI4) {
        bail!("屏幕亮度报告类型无效");
    }
    let level = u32::try_from(&level)?;
    if level > 100 {
        bail!("屏幕亮度报告超出百分比范围");
    }
    Ok(Reading {
        panel: BSTR::try_from(&panel)?.to_string(),
        current: level as u8,
        on_ac,
    })
}
fn on_ac() -> Result<bool> {
    let mut status: SYSTEM_POWER_STATUS = unsafe { std::mem::zeroed() };
    if unsafe { GetSystemPowerStatus(&mut status) } == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    super::decode_supply(status.ACLineStatus)
}
pub(super) fn read() -> Result<Reading> {
    let session = Session::connect("ROOT\\WMI")?;
    let objects = session.query("SELECT Active, InstanceName, CurrentBrightness FROM WmiMonitorBrightness WHERE Active = TRUE")?;
    let mut reading = None;
    for _ in 0..2 {
        let mut next = [None];
        let mut count = 0;
        let status = unsafe { objects.Next(1000, &mut next, &mut count) };
        status.ok()?;
        if status.0 == WBEM_S_TIMEDOUT.0 {
            bail!("屏幕亮度报告读取超时");
        }
        if count == 0 && status.0 == WBEM_S_FALSE.0 {
            break;
        }
        if count != 1 {
            bail!("屏幕亮度报告不完整");
        }
        if reading.is_some() {
            bail!("存在多个活动亮度面板，无法唯一确认目标");
        }
        reading = Some(parse(
            next[0].as_ref().context("缺少活动面板实例")?,
            on_ac()?,
        )?);
        if status.0 == WBEM_S_FALSE.0 {
            break;
        }
    }
    reading.context("当前没有可确认亮度的活动面板")
}
