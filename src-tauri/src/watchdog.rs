//! 软件风扇的独立监护进程；ARM 后承担恢复责任。
#[path = "watchdog/client.rs"]
mod client;
#[path = "watchdog/protocol.rs"]
mod protocol;
use crate::hal::{nvidia::Nvidia, oem_dchu::Dchu};
use anyhow::{Result, bail};
pub use client::Watchdog;
pub(crate) use protocol::report_failure;

pub fn run_child() -> Result<()> {
    let _lease = crate::controller_lease::ControllerLease::retain(crate::controller_lease::NAME)?;
    if !crate::hal::firmware::reviewed_dsdt() {
        bail!("监护进程未能核对逐通道自动恢复协议，未启用软件风扇曲线");
    }
    let dchu = Dchu::load()?;
    let mut nvidia = Nvidia::init().ok();
    let input = protocol::read_lines(std::io::stdin());
    protocol::run(
        input,
        || {
            let packet = dchu.read_package(12).ok()?;
            let raw = packet[18] as f64;
            if raw <= 0.0 || raw >= 150.0 {
                return None;
            }
            // 本版本 OEM GetTDP()=="0"，CPU 原始值就是其显示与控制温度。
            if nvidia.is_none() {
                nvidia = Nvidia::init().ok();
            }
            let sample = nvidia.as_ref().map(|n| n.sample()).transpose();
            if sample.is_err() {
                nvidia.take();
            }
            Some((raw, sample.ok().flatten().and_then(|value| value.temp_c)))
        },
        || restore_auto(&dchu, true),
    )
}

pub fn restore_auto(dchu: &Dchu, direct: bool) -> Result<()> {
    let mut errors = Vec::new();
    if direct {
        if let Err(error) = dchu.set_u32(0x69, crate::hal::firmware::DIRECT_FAN_MASK) {
            errors.push(format!("C1 自动恢复：{error:#}"));
        }
    }
    if let Err(error) = dchu.set_data_sub(121, 1, 0) {
        errors.push(format!("固件自动模式：{error:#}"));
    }
    let saved = dchu.write_settings(4, 5, &[0]).and_then(|_| {
        if dchu.read_settings(4, 5, 1)? != [0] {
            bail!("自动模式保存值回读失败");
        }
        Ok(())
    });
    if let Err(error) = saved {
        errors.push(format!("自动模式保存：{error:#}"));
    }
    if !errors.is_empty() {
        bail!(errors.join("；"));
    }
    Ok(())
}

#[cfg(test)]
#[path = "watchdog/restore_tests.rs"]
mod restore_tests;
