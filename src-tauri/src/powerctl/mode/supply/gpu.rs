use crate::gpu_power::{Device, mode};
use anyhow::{Result, bail};
use ccc_core::oem_supply_mode::Trigger;
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Candidate {
    pub action: Option<mode::Action>,
    pub unavailable_reason: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Review {
    pub supported: bool,
    pub current: Option<Device>,
    pub ac: Candidate,
    pub battery: Candidate,
    pub startup_ac: Candidate,
    pub startup_battery: Candidate,
    pub resume_ac: Candidate,
    pub resume_battery: Candidate,
}

impl Review {
    pub(super) fn identity(&self) -> (bool, Option<(&str, &str, bool)>) {
        (
            self.supported,
            self.current.as_ref().map(|device| {
                (
                    device.id.as_str(),
                    device.name.as_str(),
                    device.automatic_power_off,
                )
            }),
        )
    }
    pub(super) fn candidate(&self, trigger: Trigger, on_ac: bool) -> &Candidate {
        match (trigger, on_ac) {
            (Trigger::Startup, true) => &self.startup_ac,
            (Trigger::Startup, false) => &self.startup_battery,
            (Trigger::Resume, true) => &self.resume_ac,
            (Trigger::Resume, false) => &self.resume_battery,
            (Trigger::SupplyChange, true) => &self.ac,
            (Trigger::SupplyChange, false) => &self.battery,
        }
    }
    pub(super) fn candidates(&self) -> [&Candidate; 6] {
        [
            &self.ac,
            &self.battery,
            &self.startup_ac,
            &self.startup_battery,
            &self.resume_ac,
            &self.resume_battery,
        ]
    }
}

pub(super) fn review(
    supported: bool,
    devices: &[Device],
    dc_mode_power: bool,
    modes: [Option<u8>; 6],
) -> Result<Review> {
    let device = match (supported, devices) {
        (false, []) => None,
        (true, [device]) if device.healthy() => {
            mode::validate_device(device)?;
            Some(device.clone())
        }
        _ => bail!("原厂独显能力与节点集合不一致或存在故障"),
    };
    let make = |mode: Option<u8>, trigger| -> Candidate {
        let result = mode
            .map(|wanted| {
                mode::action(
                    supported,
                    device.as_ref(),
                    wanted,
                    Some(trigger),
                    dc_mode_power,
                )
            })
            .transpose();
        match result {
            Ok(value) => Candidate {
                action: value.flatten(),
                unavailable_reason: None,
            },
            Err(error) => Candidate {
                action: None,
                unavailable_reason: Some(format!("{error:#}")),
            },
        }
    };
    Ok(Review {
        supported,
        ac: make(modes[0], Trigger::SupplyChange),
        battery: make(modes[1], Trigger::SupplyChange),
        startup_ac: make(modes[2], Trigger::Startup),
        startup_battery: make(modes[3], Trigger::Startup),
        resume_ac: make(modes[4], Trigger::Resume),
        resume_battery: make(modes[5], Trigger::Resume),
        current: device,
    })
}
