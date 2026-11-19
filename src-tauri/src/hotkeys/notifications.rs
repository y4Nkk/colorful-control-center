use crate::osd;
use anyhow::{Context, Result, bail};
use ccc_core::oem_events::Notice;

pub(super) const IDENTITY: &str = "oem-state-notifications-2";

pub(super) fn message(
    notice: Notice,
    expected: &str,
    usb_keyboard: impl FnOnce() -> Result<bool>,
) -> Result<Option<osd::Message>> {
    if expected != IDENTITY {
        bail!("原厂状态通知授权已变化，请重新确认范围");
    }
    if matches!(notice, Notice::NumLock(_))
        && usb_keyboard().context("无法核对数字锁提示的USB键盘条件")?
    {
        return Ok(None);
    }
    let state = |enabled| {
        if enabled {
            "原厂报告：已开启"
        } else {
            "原厂报告：已关闭"
        }
    };
    let (title, detail) = match notice {
        Notice::NumLock(value) => ("数字锁", state(value)),
        Notice::CapsLock(value) => ("大写锁", state(value)),
        Notice::ScrollLock(value) => ("滚动锁", state(value)),
        Notice::Docking(true) => ("扩展坞", "原厂报告：已连接"),
        Notice::Docking(false) => ("扩展坞", "原厂报告：已断开"),
        Notice::Gps(value) => ("GPS", state(value)),
        Notice::SilentMode(value) => (crate::fanctl::NOISELESS_FAN_LABEL, state(value)),
        Notice::MobileNetwork(value) => ("移动网络", state(value)),
        Notice::DustRunning => ("风扇除尘", "原厂报告：正在运行"),
        Notice::TurboFan => ("Turbo 风扇", "原厂报告：已开启"),
        Notice::Charger(true) => ("充电状态", "原厂报告：开始充电"),
        Notice::Charger(false) => ("充电状态", "原厂报告：停止充电"),
    };
    Ok(Some(osd::Message::result(title, detail)))
}

#[cfg(test)]
#[path = "notification_tests.rs"]
mod tests;
