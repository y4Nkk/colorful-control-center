use super::{CoreSnapshot, Snapshot, mode_command, system};
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreTarget {
    pub revision: String,
    pub mode: u8,
    pub ac_mode: Option<u8>,
    pub turbo: bool,
    pub dtt: bool,
    pub fan: crate::fanctl::StoredCheckpoint,
    pub system: Option<(String, Option<[u32; 2]>)>,
    pub system_plans: Vec<system::RestorePlan>,
    pub energy: Option<super::super::energy::State>,
    pub gpu: Option<super::super::links::Initial>,
    pub current_gpu_power: Option<crate::gpu_power::Device>,
    pub current_gpu_clocks: crate::gpuctl::turbo::mode::ClockObservation,
    pub current_gpu_driver_identity: Option<String>,
}
impl RestoreTarget {
    pub(crate) fn bind_system(
        &mut self,
        io: &dyn crate::hal::power_plan::PowerAccess,
    ) -> Result<()> {
        if self.system_plans.is_empty() {
            return Ok(());
        }
        let mut errors = Vec::new();
        for plan in &mut self.system_plans {
            match io.aspm(u128::from_str_radix(&plan.scheme, 16)?) {
                Ok(values) => plan.current = Some(values),
                Err(error) => errors.push(format!("{}：{error:#}", plan.scheme)),
            }
        }
        if !errors.is_empty() {
            bail!(errors.join("；"));
        }
        self.revision = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&(
                "oem-mode-system-restore-1",
                &self.revision,
                &self.system_plans
            ))?)
        );
        Ok(())
    }
    pub(crate) fn bind_gpu(
        &mut self,
        node: Option<crate::gpu_power::Device>,
        clocks: crate::gpuctl::turbo::mode::ClockObservation,
        driver: Option<String>,
    ) -> Result<()> {
        self.current_gpu_power = node;
        self.current_gpu_clocks = clocks;
        self.current_gpu_driver_identity = driver;
        self.revision = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&(
                "oem-mode-gpu-restore-1",
                &self.revision,
                &self.current_gpu_power,
                &self.current_gpu_clocks,
                &self.current_gpu_driver_identity,
            ))?)
        );
        Ok(())
    }
    pub(super) fn new(snapshot: &Snapshot) -> Result<Self> {
        Ok(Self {
            revision: format!(
                "{:x}",
                Sha256::digest(serde_json::to_vec(&(
                    "oem-mode-restore-2",
                    snapshot.stored(),
                    &snapshot.links
                ))?)
            ),
            mode: snapshot.core.mode,
            ac_mode: snapshot.core.ac_mode,
            turbo: snapshot.core.preferences[0] != 0,
            dtt: snapshot.core.preferences[1] != 0,
            fan: snapshot.fan.stored(),
            system: snapshot.system.as_ref().map(system::Snapshot::summary),
            system_plans: snapshot
                .system
                .as_ref()
                .map(system::Snapshot::plans)
                .unwrap_or_default(),
            energy: snapshot.energy,
            gpu: snapshot.links.clone(),
            current_gpu_power: None,
            current_gpu_driver_identity: None,
            current_gpu_clocks: if snapshot
                .links
                .as_ref()
                .is_some_and(|initial| initial.clocks.is_some())
            {
                crate::gpuctl::turbo::mode::ClockObservation::Unavailable(
                    "需要完整GPU恢复预览".into(),
                )
            } else {
                crate::gpuctl::turbo::mode::ClockObservation::Untracked
            },
        })
    }
}

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
    pub current_fan_offset: u8,
    pub initial_fan_offset: u8,
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
        if self.core.mode > 4
            || self.core.ac_mode.is_some() && self.energy.is_none()
            || self.core.preferences.iter().any(|value| *value > 1)
            || self.core.command != mode_command(self.core.mode, self.core.preferences)
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
            "fan":{"software":null,"package":null,"rpmPairs":[],"fields":[{"offset":5,"data":[0]}, {"offset":7,"data":[5]}],"mode":0,"offset":5},
            "system":{"active":"381b4222f69441f09685ff5bb260df2e","aspm":{"381b4222f69441f09685ff5bb260df2e":[0,2]}},"energy":{"hardwarePState":true,"energyStar":null}});
        let mut current = initial.clone();
        current["core"] = serde_json::json!({"mode":3,"command":3,"preferences":[0,0],"acMode":2});
        current["fan"]["fields"][1]["data"] = serde_json::json!([15]);
        current["fan"]["offset"] = 15.into();
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
            "fan":{"software":null,"package":null,"rpmPairs":[],"fields":[{"offset":5,"data":[0]}, {"offset":7,"data":[0]}],"mode":0,"offset":0},
            "system":{"active":"00000000000000000000000000000001","aspm":{"00000000000000000000000000000001":[0,2]}},"energy":{"hardwarePState":true,"energyStar":null}});
        let saved: StoredModeSnapshot = serde_json::from_value(data.clone()).unwrap();
        let restored = saved.restore().unwrap();
        assert!(restored.stored() == saved);
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
