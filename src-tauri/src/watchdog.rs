//! 软件风扇的独立监护进程；ARM 后承担恢复责任。
#[path = "watchdog/client.rs"]
mod client;
#[path = "watchdog/protocol.rs"]
mod protocol;
use crate::hal::{firmware, nvidia::Nvidia, oem_dchu::Dchu};
use anyhow::{Context, Result, bail, ensure};
use ccc_core::fan_sensors::FanReadings;
pub use client::Watchdog;
pub(crate) use protocol::report_failure;

pub fn run_child() -> Result<()> {
    let _lease = crate::controller_lease::ControllerLease::retain(crate::controller_lease::NAME)?;
    ensure!(firmware::reviewed_dsdt(), firmware::FAN_CONTROL_UNVERIFIED);
    let reader = firmware::DIRECT_FAN_READER.context(firmware::FAN_CONTROL_UNVERIFIED)?;
    let dchu = Dchu::load()?;
    firmware::verify_direct_release(&dchu, Some(reader))?;
    let sensors = Dchu::load()?;
    let mut nvidia = None;
    let sample = protocol::monitor_samples(move || {
        let packet = sensors.read_package(12).ok()?;
        let reading = FanReadings::decode(&packet, firmware::DIRECT_FAN_MASK as u8)?;
        let cpu = reading.cpu.temp_c?;
        if nvidia.is_none() {
            nvidia = Nvidia::init().ok();
        }
        let sample = nvidia.as_ref().map(|n| n.sample()).transpose();
        if sample.is_err() {
            nvidia.take();
        }
        Some((cpu, sample.ok().flatten().and_then(|value| value.temp_c)))
    })?;
    protocol::run(
        protocol::read_lines(std::io::stdin()),
        sample,
        || firmware::verify_direct_release(&dchu, Some(reader)),
        || restore_auto(&dchu, Some(reader)),
    )
}

pub(crate) fn restore_auto(dchu: &Dchu, reader: Option<firmware::ReadDirectControl>) -> Result<()> {
    let mut errors = Vec::new();
    if let Err(error) = dchu.set_u32(0x69, firmware::DIRECT_FAN_MASK) {
        errors.push(format!("C1 自动恢复：{error:#}"));
    }
    if let Err(error) = dchu.set_fan_mode_verified(0) {
        errors.push(format!("切换风扇到「自动」失败：{error:#}"));
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
    if let Err(error) = firmware::verify_direct_release(dchu, reader) {
        errors.push(format!("尚未确认所有风扇已退出软件调速：{error:#}"));
    }
    if !errors.is_empty() {
        bail!(errors.join("；"));
    }
    Ok(())
}

#[cfg(test)]
#[path = "watchdog/restore_tests.rs"]
mod restore_tests;
