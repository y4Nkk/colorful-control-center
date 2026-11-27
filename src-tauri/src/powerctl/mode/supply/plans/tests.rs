use super::*;
use crate::hal::power_plan::{BatteryAction, BatteryLevel};
use std::{collections::BTreeMap, sync::Mutex};

struct Memory(Mutex<BTreeMap<u128, [u32; 2]>>);
impl PowerAccess for Memory {
    fn lid_action(&self, _: u128) -> Result<[u32; 2]> {
        anyhow::bail!("此隔离提供者未注入合盖计划动作")
    }
    fn active(&self) -> Result<u128> {
        bail!("未注入活动计划")
    }
    fn schemes(&self) -> Result<Vec<u128>> {
        bail!("未注入计划枚举")
    }
    fn scheme_name(&self, _: u128) -> Result<String> {
        bail!("供电模式测试未注入计划名称")
    }
    fn check_active_scheme_write(&self) -> Result<()> {
        bail!("供电模式测试未注入计划切换权限")
    }
    fn battery_action(&self, _: u128, _: BatteryAction) -> Result<u32> {
        bail!("未注入电池动作")
    }
    fn battery_level(&self, _: u128, _: BatteryLevel) -> Result<[u32; 2]> {
        bail!("未注入电池阈值")
    }
    fn set_battery_level(&self, _: u128, _: BatteryLevel, _: bool, _: u32) -> Result<()> {
        bail!("未注入电池阈值")
    }
    fn usb(&self, _: u128) -> Result<[u32; 2]> {
        bail!("未注入 USB 设置")
    }
    fn set_usb(&self, _: u128, _: bool, _: u32) -> Result<()> {
        bail!("未注入 USB 设置")
    }
    fn brightness(&self, _: u128) -> Result<[u32; 2]> {
        bail!("未注入亮度设置")
    }
    fn set_brightness(&self, _: u128, _: bool, _: u32) -> Result<()> {
        bail!("未注入亮度设置")
    }
    fn activate(&self, _: u128) -> Result<()> {
        bail!("未注入计划切换")
    }
    fn aspm(&self, scheme: u128) -> Result<[u32; 2]> {
        self.0
            .lock()
            .unwrap()
            .get(&scheme)
            .copied()
            .ok_or_else(|| anyhow::anyhow!("计划不存在"))
    }
    fn set_aspm(&self, scheme: u128, ac: bool, value: u32) -> Result<()> {
        self.0
            .lock()
            .unwrap()
            .get_mut(&scheme)
            .ok_or_else(|| anyhow::anyhow!("计划不存在"))?[usize::from(!ac)] = value;
        Ok(())
    }
}

fn snapshot() -> StoredModeSnapshot {
    serde_json::from_value(serde_json::json!({
        "core":{"mode":2,"command":2,"preferences":[0,0],"acMode":null},
        "fan":{"software":null,"package":null,"rpmPairs":null,"fields":[{"offset":5,"data":[0]},{"offset":7,"data":[0]}],"mode":0,"offsetRaw":0},
        "system":{"active":format!("{:032x}", SCHEMES[0]),"aspm":{"381b4222f69441f09685ff5bb260df2e":[0,1]}},"energy":{"hardwarePState":true,"energyStar":null}
    })).unwrap()
}
fn fixture() -> (Memory, Review, StoredModeSnapshot) {
    let target = crate::powerctl::mode::Target {
        from_mode: None,
        gpu: None,
        revision: None,
        unavailable_reason: None,
        ac_mode: None,
        energy: Some(crate::powerctl::mode::energy::State {
            hardware_p_state: true,
            energy_star: None,
        }),
        mode: 2,
        turbo: false,
        dtt: false,
        fan: crate::fanctl::ModePolicy::explicit(0, 0, Some(0)),
        aspm: Some([2, 2]),
        power_plan: Some(2),
    };
    let review = Review {
        device_identity: "a".repeat(64),
        identity: "b".repeat(64),
        revision: "r".into(),
        current_mode: 2,
        ac_mode: 2,
        on_ac: Some(true),
        ac: Some(target.clone()),
        battery: Some(target.clone()),
        startup_ac: Some(target.clone()),
        startup_battery: Some(target.clone()),
        resume_ac: Some(target.clone()),
        resume_battery: Some(target.clone()),
        targets: vec![target],
        software_fan_active: false,
        current_energy: crate::powerctl::mode::energy::State {
            hardware_p_state: true,
            energy_star: None,
        },
        dc_mode_power: true,
        gpu: super::super::gpu::review(false, &[], true, [None; 6]).unwrap(),
        clocks: Some(crate::gpuctl::turbo::mode::review(
            crate::gpuctl::turbo::mode::test_policy(),
            [None; 6],
        )),
    };
    (
        Memory(Mutex::new(
            [(SCHEMES[0], [0, 1]), (SCHEMES[2], [1, 2])].into(),
        )),
        review,
        snapshot(),
    )
}

#[test]
fn captures_all_affected_plans_and_restores_non_initial_plans() {
    let (io, review, before) = fixture();
    let initial = initial(&io, &review, &before).unwrap();
    assert_eq!(initial.len(), 2);
    io.0.lock().unwrap().insert(SCHEMES[0], [2, 2]);
    io.0.lock().unwrap().insert(SCHEMES[2], [0, 0]);
    let expected = current(&io, &initial).unwrap();
    restore_other_plans(&io, &initial, &expected, &before, &mut || Ok(())).unwrap();
    assert_eq!(io.aspm(SCHEMES[2]).unwrap(), [1, 2]);
    assert_eq!(io.aspm(SCHEMES[0]).unwrap(), [2, 2]);
}

#[test]
fn stale_plan_refuses_restore_and_partial_restore_is_retryable() {
    let (io, review, before) = fixture();
    let initial = initial(&io, &review, &before).unwrap();
    io.0.lock().unwrap().insert(SCHEMES[2], [0, 0]);
    let expected = current(&io, &initial).unwrap();
    io.0.lock().unwrap().insert(SCHEMES[2], [2, 0]);
    assert!(restore_other_plans(&io, &initial, &expected, &before, &mut || Ok(())).is_err());
    assert_eq!(io.aspm(SCHEMES[2]).unwrap(), [2, 0]);
    let revised = current(&io, &initial).unwrap();
    let mut steps = 0;
    assert!(
        restore_other_plans(&io, &initial, &revised, &before, &mut || {
            steps += 1;
            if steps == 2 { bail!("撤销") } else { Ok(()) }
        })
        .is_err()
    );
    assert_eq!(io.aspm(SCHEMES[2]).unwrap(), [1, 0]);
    let revised = current(&io, &initial).unwrap();
    restore_other_plans(&io, &initial, &revised, &before, &mut || Ok(())).unwrap();
    assert_eq!(io.aspm(SCHEMES[2]).unwrap(), [1, 2]);
}

#[test]
fn invalid_or_changed_plan_records_are_rejected() {
    let (io, review, before) = fixture();
    let mut initial = initial(&io, &review, &before).unwrap();
    initial.push(initial[0].clone());
    assert!(validate(&initial).is_err());
    io.0.lock().unwrap().insert(SCHEMES[0], [2, 1]);
    assert!(super::initial(&io, &review, &before).is_err());
}

fn multi_plan_snapshot() -> StoredModeSnapshot {
    let mut data = serde_json::to_value(snapshot()).unwrap();
    data["system"]["aspm"]
        .as_object_mut()
        .unwrap()
        .insert(format!("{:032x}", SCHEMES[2]), serde_json::json!([1, 2]));
    serde_json::from_value(data).unwrap()
}

#[test]
fn initial_capture_checks_every_owned_plan_even_when_it_is_not_a_future_activation_target() {
    let (io, mut review, _) = fixture();
    review.targets[0].power_plan = None;
    for target in [
        &mut review.ac,
        &mut review.battery,
        &mut review.startup_ac,
        &mut review.startup_battery,
        &mut review.resume_ac,
        &mut review.resume_battery,
    ]
    .into_iter()
    .flatten()
    {
        target.power_plan = None;
    }
    let before = multi_plan_snapshot();
    let values = initial(&io, &review, &before).unwrap();
    assert_eq!(values.len(), 2);
    validate_initial(&before, &values).unwrap();
    io.0.lock().unwrap().insert(SCHEMES[2], [0, 0]);
    assert!(initial(&io, &review, &before).is_err());
    let changed = current(&io, &values).unwrap();
    assert!(validate_initial(&before, &changed).is_err());
}

#[test]
fn outer_restore_does_not_repeat_any_plan_already_restored_by_the_core_snapshot() {
    let (io, review, _) = fixture();
    let before = multi_plan_snapshot();
    let initial = initial(&io, &review, &before).unwrap();
    io.0.lock().unwrap().insert(SCHEMES[0], [2, 2]);
    io.0.lock().unwrap().insert(SCHEMES[2], [0, 0]);
    let expected = current(&io, &initial).unwrap();
    // 核心快照先恢复自己拥有的两个计划；外围仍携带执行前的预览。
    io.0.lock().unwrap().insert(SCHEMES[0], [0, 1]);
    io.0.lock().unwrap().insert(SCHEMES[2], [1, 2]);
    restore_other_plans(&io, &initial, &expected, &before, &mut || {
        anyhow::bail!("已归属计划不应再次请求写入许可")
    })
    .unwrap();
    assert_eq!(io.aspm(SCHEMES[0]).unwrap(), [0, 1]);
    assert_eq!(io.aspm(SCHEMES[2]).unwrap(), [1, 2]);
}
