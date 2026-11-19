use super::{ProfileSettings, fan_target};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProfileGroup {
    Performance,
    Display,
    Lighting,
    Power,
    Devices,
}

impl ProfileGroup {
    pub const ALL: [Self; 5] = [
        Self::Performance,
        Self::Display,
        Self::Lighting,
        Self::Power,
        Self::Devices,
    ];

    pub fn includes(self, s: &ProfileSettings) -> bool {
        match self {
            Self::Performance => {
                s.oem_power_mode.is_some()
                    || s.performance_options.is_some()
                    || fan_target(s).is_some()
                    || s.runtime_cpu.is_some()
                    || s.pl1_w.is_some()
                    || s.pl2_w.is_some()
                    || s.core_offset_mv.is_some()
                    || s.gt_offset_mv.is_some()
                    || s.gpu.is_some()
            }
            Self::Display => {
                s.display.is_some() || s.gpu_power.is_some() || s.graphics_mode.is_some()
            }
            Self::Lighting => {
                s.light_bar.is_some()
                    || s.keyboard_lighting.is_some()
                    || s.multi_zone_lighting.is_some()
                    || s.lighting_options.is_some()
                    || s.perkey_lighting.is_some()
            }
            Self::Power => {
                s.power_delivery.is_some()
                    || s.usb_policy.is_some()
                    || s.battery_charge.is_some()
                    || s.battery_energy.is_some()
                    || s.lan_policy.is_some()
            }
            Self::Devices => {
                s.dust_schedule.is_some()
                    || s.keyboard.is_some()
                    || s.microphones.is_some()
                    || s.touchpad.is_some()
                    || s.keymap.is_some()
            }
        }
    }

    pub fn captured(settings: &ProfileSettings) -> Vec<Self> {
        Self::ALL
            .into_iter()
            .filter(|group| group.includes(settings))
            .collect()
    }
}
