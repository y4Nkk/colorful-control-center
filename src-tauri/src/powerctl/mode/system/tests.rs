use super::*;
use std::{collections::BTreeMap, sync::Mutex};
struct Memory(Mutex<State>);
struct State {
    active: u128,
    schemes: BTreeMap<u128, [u32; 2]>,
    writes: usize,
    fail_at: Option<usize>,
    persistent_fail: bool,
}
impl PowerAccess for Memory {
    fn lid_action(&self, _: u128) -> Result<[u32; 2]> {
        anyhow::bail!("此隔离提供者未注入合盖计划动作")
    }
    fn battery_action(&self, _: u128, _: crate::hal::power_plan::BatteryAction) -> Result<u32> {
        bail!("此隔离提供者未注入电池计划动作")
    }
    fn battery_level(&self, _: u128, _: crate::hal::power_plan::BatteryLevel) -> Result<[u32; 2]> {
        bail!("此隔离提供者未注入电池计划阈值")
    }
    fn set_battery_level(
        &self,
        _: u128,
        _: crate::hal::power_plan::BatteryLevel,
        _: bool,
        _: u32,
    ) -> Result<()> {
        bail!("此隔离提供者未注入电池计划阈值")
    }
    fn brightness(&self, _: u128) -> anyhow::Result<[u32; 2]> {
        anyhow::bail!("此隔离提供者未注入屏幕亮度字段")
    }
    fn set_brightness(&self, _: u128, _: bool, _: u32) -> anyhow::Result<()> {
        anyhow::bail!("此隔离提供者未注入屏幕亮度字段")
    }

    fn schemes(&self) -> Result<Vec<u128>> {
        Ok(self.0.lock().unwrap().schemes.keys().copied().collect())
    }
    fn usb(&self, _: u128) -> Result<[u32; 2]> {
        bail!("模式隔离提供者未注入USB字段")
    }
    fn set_usb(&self, _: u128, _: bool, _: u32) -> Result<()> {
        bail!("模式隔离提供者未注入USB字段")
    }
    fn active(&self) -> Result<u128> {
        Ok(self.0.lock().unwrap().active)
    }
    fn aspm(&self, scheme: u128) -> Result<[u32; 2]> {
        self.0
            .lock()
            .unwrap()
            .schemes
            .get(&scheme)
            .copied()
            .ok_or_else(|| anyhow::anyhow!("计划不存在"))
    }
    fn set_aspm(&self, scheme: u128, ac: bool, value: u32) -> Result<()> {
        let mut state = self.0.lock().unwrap();
        state.schemes.get_mut(&scheme).unwrap()[usize::from(!ac)] = value;
        state.finish()
    }
    fn activate(&self, scheme: u128) -> Result<()> {
        let mut state = self.0.lock().unwrap();
        state.active = scheme;
        state.finish()
    }
}
impl State {
    fn finish(&mut self) -> Result<()> {
        let at = self.writes;
        self.writes += 1;
        if self.persistent_fail || self.fail_at == Some(at) {
            self.fail_at = None;
            bail!("电源 API 部分成功后故障");
        }
        Ok(())
    }
}
fn fixture() -> (Memory, Target) {
    let io = Memory(Mutex::new(State {
        active: SCHEMES[0],
        schemes: [(SCHEMES[0], [0, 1]), (SCHEMES[2], [1, 2])].into(),
        writes: 0,
        fail_at: None,
        persistent_fail: false,
    }));
    let target = Target {
        from_mode: None,
        gpu: None,
        revision: None,
        unavailable_reason: None,
        energy: None,
        ac_mode: None,
        mode: 4,
        turbo: false,
        dtt: false,
        fan_mode: 0,
        fan_saved_mode: 0,
        fan_offset_pct: 0,
        aspm: Some([2, 2]),
        power_plan: Some(2),
    };
    (io, target)
}
#[test]
fn aspm_edits_original_active_scheme_before_switching_and_restores_exact_state() {
    let (io, target) = fixture();
    let before = capture(&io, &target).unwrap();
    apply(&io, &before, &target, &mut || Ok(())).unwrap();
    assert_eq!(io.active().unwrap(), SCHEMES[2]);
    assert_eq!(io.aspm(SCHEMES[0]).unwrap(), [2, 2]);
    assert_eq!(io.aspm(SCHEMES[2]).unwrap(), [1, 2]);
    restore(&io, &before).unwrap();
    assert_eq!(io.active().unwrap(), SCHEMES[0]);
    assert_eq!(io.aspm(SCHEMES[0]).unwrap(), [0, 1]);
}

#[test]
fn merged_snapshots_keep_first_values_and_roundtrip_all_touched_plans() {
    let (io, mut target) = fixture();
    let mut initial = capture(&io, &target).unwrap().unwrap();
    apply(&io, &Some(initial.clone()), &target, &mut || Ok(())).unwrap();
    let second = capture(&io, &target).unwrap().unwrap();
    initial.merge(&second);
    target.aspm = Some([0, 0]);
    apply(&io, &Some(second), &target, &mut || Ok(())).unwrap();
    let third = capture(&io, &target).unwrap().unwrap();
    initial.merge(&third);
    let encoded = serde_json::to_value(initial.stored()).unwrap();
    assert_eq!(encoded["aspm"].as_object().unwrap().len(), 2);
    let saved: StoredSnapshot = serde_json::from_value(encoded).unwrap();
    restore(&io, &Some(saved.restore().unwrap())).unwrap();
    assert_eq!(io.active().unwrap(), SCHEMES[0]);
    assert_eq!(io.aspm(SCHEMES[0]).unwrap(), [0, 1]);
    assert_eq!(io.aspm(SCHEMES[2]).unwrap(), [1, 2]);
    assert!(
        serde_json::from_value::<StoredSnapshot>(
            serde_json::json!({"active":format!("{:032x}",SCHEMES[0]),"aspm":[0,1]})
        )
        .is_err()
    );
}

#[test]
fn multi_plan_restore_attempts_every_field_after_failures_and_retains_retryable_values() {
    let (io, target) = fixture();
    let mut initial = capture(&io, &target).unwrap().unwrap();
    apply(&io, &Some(initial.clone()), &target, &mut || Ok(())).unwrap();
    let second = capture(&io, &target).unwrap().unwrap();
    initial.merge(&second);
    apply(&io, &Some(second), &target, &mut || Ok(())).unwrap();
    let writes = {
        let mut state = io.0.lock().unwrap();
        state.persistent_fail = true;
        state.writes
    };
    let error = restore(&io, &Some(initial.clone()))
        .unwrap_err()
        .to_string();
    assert_eq!(error.matches("部分成功后故障").count(), 5);
    assert_eq!(io.0.lock().unwrap().writes - writes, 5);
    io.0.lock().unwrap().persistent_fail = false;
    restore(&io, &Some(initial)).unwrap();
    assert_eq!(io.active().unwrap(), SCHEMES[0]);
    assert_eq!(io.aspm(SCHEMES[0]).unwrap(), [0, 1]);
    assert_eq!(io.aspm(SCHEMES[2]).unwrap(), [1, 2]);
}
#[test]
fn all_partial_write_failures_have_complete_compensation() {
    for at in 0..3 {
        let (io, target) = fixture();
        let before = capture(&io, &target).unwrap();
        io.0.lock().unwrap().fail_at = Some(at);
        assert!(apply(&io, &before, &target, &mut || Ok(())).is_err());
        restore(&io, &before).unwrap();
        assert_eq!(io.active().unwrap(), SCHEMES[0]);
        assert_eq!(io.aspm(SCHEMES[0]).unwrap(), [0, 1]);
    }
}
#[test]
fn restore_attempts_both_values_and_active_plan_and_can_retry() {
    let (io, target) = fixture();
    let before = capture(&io, &target).unwrap();
    apply(&io, &before, &target, &mut || Ok(())).unwrap();
    io.0.lock().unwrap().persistent_fail = true;
    let writes = io.0.lock().unwrap().writes;
    let error = restore(&io, &before).unwrap_err().to_string();
    assert_eq!(error.matches("部分成功后故障").count(), 3);
    assert_eq!(io.0.lock().unwrap().writes - writes, 3);
    io.0.lock().unwrap().persistent_fail = false;
    restore(&io, &before).unwrap();
    assert_eq!(io.active().unwrap(), SCHEMES[0]);
}
#[test]
fn missing_scheme_rejects_before_mutation_and_no_policy_uses_no_system_api() {
    let (io, mut target) = fixture();
    target.power_plan = Some(1);
    assert!(capture(&io, &target).is_err());
    assert_eq!(io.0.lock().unwrap().writes, 0);
    target.aspm = None;
    target.power_plan = None;
    let unavailable = crate::hal::power_plan::UnavailablePower;
    let before = capture(&unavailable, &target).unwrap();
    assert!(before.is_none());
    apply(&unavailable, &before, &target, &mut || Ok(())).unwrap();
    restore(&unavailable, &before).unwrap();
}
