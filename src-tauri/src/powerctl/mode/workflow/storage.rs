use super::{CoreSnapshot, Snapshot, mode_command, system};
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RecoveryPreview {
    pub current_mode: u8,
    pub initial_mode: u8,
    pub current_ac_mode: Option<u8>,
    pub initial_ac_mode: Option<u8>,
    pub current_turbo: bool,
    pub initial_turbo: bool,
    pub current_dtt: bool,
    pub initial_dtt: bool,
    pub current_fan: crate::fanctl::StoredCheckpoint,
    pub initial_fan: crate::fanctl::StoredCheckpoint,
    pub current_fan_mode: u8,
    pub initial_fan_mode: u8,
    pub current_fan_offset: Option<u8>,
    pub initial_fan_offset: Option<u8>,
    pub current_software_fan: bool,
    pub initial_software_fan: bool,
    pub fan_saved_settings_changed: bool,
    pub current_system: Option<(String, Option<[u32; 2]>)>,
    pub initial_system: Option<(String, Option<[u32; 2]>)>,
    pub plan_changes: Vec<super::super::supply::plans::PlanChange>,
    pub current_energy: Option<super::super::energy::State>,
    pub initial_energy: Option<super::super::energy::State>,
    pub current_gpu_power: Option<crate::gpu_power::mode::State>,
    pub initial_gpu_power: Option<crate::gpu_power::mode::State>,
    pub current_gpu_clocks: Option<crate::gpuctl::turbo::mode::ClockObservation>,
    pub initial_gpu_clocks: Option<crate::gpuctl::turbo::mode::ClockBaseline>,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct StoredModeSnapshot {
    core: StoredCore,
    fan: crate::fanctl::StoredCheckpoint,
    system: Option<system::StoredSnapshot>,
    #[serde(deserialize_with = "Option::deserialize")]
    energy: Option<super::super::energy::State>,
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredCore {
    mode: u8,
    command: u32,
    preferences: [u8; 2],
    #[serde(deserialize_with = "Option::deserialize")]
    ac_mode: Option<u8>,
}
impl Snapshot {
    pub(crate) fn stored(&self) -> StoredModeSnapshot {
        StoredModeSnapshot {
            core: StoredCore {
                mode: self.core.mode,
                command: self.core.command,
                preferences: self.core.preferences,
                ac_mode: self.core.ac_mode,
            },
            fan: self.fan.stored(),
            system: self.system.as_ref().map(system::Snapshot::stored),
            energy: self.energy,
        }
    }
}
impl StoredModeSnapshot {
    pub(crate) fn system_plan_values(
        &self,
    ) -> Result<std::collections::BTreeMap<String, [u32; 2]>> {
        Ok(self
            .restore()?
            .system
            .as_ref()
            .map(system::Snapshot::aspm_values)
            .unwrap_or_default())
    }
    pub(crate) fn energy(&self) -> Result<super::super::energy::State> {
        self.energy
            .ok_or_else(|| anyhow::anyhow!("供电模式检查点缺少EnergyStar所有权"))
    }
    pub(crate) fn current_mode(&self) -> u8 {
        self.core.mode
    }
    pub(crate) fn restore(&self) -> Result<Snapshot> {
        if let Some(energy) = self.energy {
            energy.validate()?;
        }
        let expected = mode_command(self.core.mode, self.core.preferences);
        let valid_command = if self.core.mode == 3 {
            self.core.command & 0x3f == 3 && self.core.command & !expected == 0
        } else {
            self.core.command == expected
        };
        if !ccc_core::oem_events::is_power_mode(self.core.mode)
            || self.core.ac_mode.is_some() && self.energy.is_none()
            || self.core.preferences.iter().any(|value| *value > 1)
            || !valid_command
        {
            bail!("原厂模式恢复记录无效");
        }
        Ok(Snapshot {
            core: CoreSnapshot {
                mode: self.core.mode,
                command: self.core.command,
                preferences: self.core.preferences,
                ac_mode: self.core.ac_mode,
            },
            fan: self.fan.restore()?,
            system: self
                .system
                .as_ref()
                .map(system::StoredSnapshot::restore)
                .transpose()?,
            energy: self.energy,
            links: None,
        })
    }
    pub(crate) fn recovery_preview(&self, current: &Self) -> Result<RecoveryPreview> {
        let initial = self.restore()?;
        let current = current.restore()?;
        Ok(RecoveryPreview {
            current_mode: current.core.mode,
            initial_mode: initial.core.mode,
            current_ac_mode: current.core.ac_mode,
            initial_ac_mode: initial.core.ac_mode,
            current_turbo: current.core.preferences[0] != 0,
            initial_turbo: initial.core.preferences[0] != 0,
            current_dtt: current.core.preferences[1] != 0,
            initial_dtt: initial.core.preferences[1] != 0,
            current_fan: current.fan.stored(),
            initial_fan: initial.fan.stored(),
            current_fan_mode: current.fan.mode(),
            initial_fan_mode: initial.fan.mode(),
            current_fan_offset: current.fan.offset(),
            initial_fan_offset: initial.fan.offset(),
            current_software_fan: current.fan.software_curves().is_some(),
            initial_software_fan: initial.fan.software_curves().is_some(),
            fan_saved_settings_changed: initial.fan.saved_settings_differ(&current.fan),
            current_system: current.system.as_ref().map(system::Snapshot::summary),
            initial_system: initial.system.as_ref().map(system::Snapshot::summary),
            plan_changes: Vec::new(),
            current_energy: current.energy,
            initial_energy: initial.energy,
            current_gpu_power: None,
            initial_gpu_power: None,
            current_gpu_clocks: None,
            initial_gpu_clocks: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recovery_preview_preserves_option_preferences_and_complete_distinct_fan_snapshots() {
        let initial = serde_json::json!({"core":{"mode":2,"command":194,"preferences":[1,1],"acMode":3},
            "fan":{"software":null,"package":null,"rpmPairs":null,"fields":[{"offset":5,"data":[0]}, {"offset":7,"data":[5]}],"mode":0,"offsetRaw":13},
            "system":{"active":"381b4222f69441f09685ff5bb260df2e","aspm":{"381b4222f69441f09685ff5bb260df2e":[0,2]}},"energy":{"hardwarePState":true,"energyStar":null}});
        let mut current = initial.clone();
        current["core"] = serde_json::json!({"mode":3,"command":3,"preferences":[0,0],"acMode":2});
        current["fan"]["fields"][1]["data"] = serde_json::json!([15]);
        current["fan"]["offsetRaw"] = 38.into();
        let saved: StoredModeSnapshot = serde_json::from_value(initial.clone()).unwrap();
        let now: StoredModeSnapshot = serde_json::from_value(current.clone()).unwrap();
        let value = serde_json::to_value(saved.recovery_preview(&now).unwrap()).unwrap();
        assert_eq!(value["currentMode"], 3);
        assert_eq!(value["initialMode"], 2);
        assert_eq!(value["currentAcMode"], 2);
        assert_eq!(value["initialAcMode"], 3);
        assert_eq!(value["currentTurbo"], false);
        assert_eq!(value["initialTurbo"], true);
        assert_eq!(value["currentDtt"], false);
        assert_eq!(value["initialDtt"], true);
        assert_eq!(value["initialFan"], initial["fan"]);
        assert_eq!(value["currentFan"], current["fan"]);
    }
    #[test]
    fn strict_mode_snapshot_rejects_inconsistent_commands_and_unknown_fields() {
        let mut data = serde_json::json!({"core":{"mode":2,"command":194,"preferences":[1,1],"acMode":null},
            "fan":{"software":null,"package":null,"rpmPairs":null,"fields":[{"offset":5,"data":[0]}, {"offset":7,"data":[0]}],"mode":0,"offsetRaw":0},
            "system":{"active":"00000000000000000000000000000001","aspm":{"00000000000000000000000000000001":[0,2]}},"energy":{"hardwarePState":true,"energyStar":null}});
        let saved: StoredModeSnapshot = serde_json::from_value(data.clone()).unwrap();
        let restored = saved.restore().unwrap();
        assert!(restored.stored() == saved);
        let mut unknown = data.clone();
        unknown["core"] =
            serde_json::json!({"mode":4,"command":4,"preferences":[0,0],"acMode":null});
        assert!(
            serde_json::from_value::<StoredModeSnapshot>(unknown)
                .unwrap()
                .restore()
                .is_err()
        );
        let mut old = data.clone();
        old["core"].as_object_mut().unwrap().remove("acMode");
        assert!(serde_json::from_value::<StoredModeSnapshot>(old).is_err());
        let mut old = data.clone();
        old.as_object_mut().unwrap().remove("energy");
        assert!(serde_json::from_value::<StoredModeSnapshot>(old).is_err());
        let mut unowned = data.clone();
        unowned["core"]["acMode"] = 2.into();
        unowned["energy"] = serde_json::Value::Null;
        assert!(
            serde_json::from_value::<StoredModeSnapshot>(unowned)
                .unwrap()
                .restore()
                .is_err()
        );
        data["core"]["command"] = 2.into();
        assert!(
            serde_json::from_value::<StoredModeSnapshot>(data.clone())
                .unwrap()
                .restore()
                .is_err()
        );
        data["extra"] = true.into();
        assert!(serde_json::from_value::<StoredModeSnapshot>(data).is_err());
    }
}
