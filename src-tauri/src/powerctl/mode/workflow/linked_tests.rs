use super::*;
#[path = "../../../profiles/apply/fixtures.rs"]
#[allow(dead_code)]
mod fixtures;
use fixtures::Fixture;
#[path = "../links/plan_tests.rs"]
mod plan_tests;

fn setup() -> Fixture {
    let f = Fixture::new();
    {
        let mut state = f.device.0.lock().unwrap();
        state.pages[1][1] = 2;
        state.bios[0] = 1;
        state.bios[11] |= 16;
        state.bios[16] |= 8;
        state.bios[19] |= 2;
        state.bios[20] |= 0x40;
        let mut aspm = [0; 256];
        aspm[3..5].copy_from_slice(&[2, 1]);
        state.policy_tables.insert(15, aspm);
        let mut plans = [0; 256];
        plans[2] = 2;
        state.policy_tables.insert(29, plans);
    }
    f
}

fn mark(f: &Fixture, marker: &str) {
    f.device.0.lock().unwrap().events.push(marker.into());
}

fn position(f: &Fixture, prefix: &str) -> usize {
    f.device
        .0
        .lock()
        .unwrap()
        .events
        .iter()
        .position(|event| event.starts_with(prefix))
        .unwrap()
}

#[test]
fn linked_effects_run_after_mode_and_before_fan_and_system_policy() {
    let f = setup();
    let _gate = f.hal.begin_write().unwrap();
    let plan = checkpoint(&f.hal, 3).unwrap();
    plan.execute_linked(
        &f.hal,
        &mut || Ok(()),
        &mut |permit| {
            permit()?;
            mark(&f, "GPU_AND_ENERGY");
            assert_eq!(f.device.0.lock().unwrap().pages[1][1], 3);
            Ok(())
        },
        &mut |_| Ok(()),
        &mut || {
            mark(&f, "LINK_ROLLBACK");
            Ok(())
        },
    )
    .unwrap();
    assert!(position(&f, "MODE:3") < position(&f, "GPU_AND_ENERGY"));
    assert!(position(&f, "GPU_AND_ENERGY") < position(&f, "PAGE4:5:"));
    assert!(position(&f, "GPU_AND_ENERGY") < position(&f, "ASPM"));
    assert!(position(&f, "PLAN:") < position(&f, "PAGE4:5:"));
    assert!(
        !f.device
            .0
            .lock()
            .unwrap()
            .events
            .iter()
            .any(|e| e == "LINK_ROLLBACK")
    );
}

#[test]
fn preset_without_fan_memory_leaves_mode_and_offset_to_firmware() {
    let f = setup();
    {
        let mut state = f.device.0.lock().unwrap();
        state.bios[20] = 1;
        state.pages[1][1] = 3;
        state.pages[4][7] = 40;
        state.pages[4][9] = 1;
        state.fan_mode = 8;
        state.offset_raw = 0x4c;
        state.ignore_fan_write = true;
    }
    let _gate = f.hal.begin_write().unwrap();
    let plan = checkpoint(&f.hal, 0).unwrap();
    assert_eq!(plan.target.fan, fanctl::ModePolicy::Firmware { saved_mode: 0 });
    plan.execute_linked(
        &f.hal,
        &mut || Ok(()),
        &mut |_| {
            f.device.0.lock().unwrap().fan_mode = 2;
            Ok(())
        },
        &mut |_| Ok(()),
        &mut || Ok(()),
    )
    .unwrap();
    {
        let mut state = f.device.0.lock().unwrap();
        assert_eq!(state.pages[1][1], 0);
        assert_eq!(state.fan_mode, 2);
        assert_eq!(state.fan_command, None);
        assert_eq!(state.offset_raw, 0x4c);
        assert_eq!((state.pages[4][5], state.pages[4][7]), (0, 40));
        state.ignore_fan_write = false;
    }
    let entertainment = checkpoint(&f.hal, 3).unwrap();
    assert_eq!(entertainment.target.fan, fanctl::ModePolicy::explicit(8, 0, None));
    entertainment.execute(&f.hal).unwrap();
    assert_eq!(f.device.0.lock().unwrap().fan_command, Some(8));
}

#[test]
fn dust_starting_after_preview_rejects_mode_before_any_write() {
    let f = setup();
    f.device.0.lock().unwrap().bios[10] = 128;
    let _gate = f.hal.begin_write().unwrap();
    let plan = checkpoint(&f.hal, 3).unwrap();
    f.device.0.lock().unwrap().dust_running = true;
    let error = plan.execute_guarded(&f.hal, &mut || Ok(())).unwrap_err();
    assert!(error.to_string().contains("固件正在除尘"));
    assert!(f.device.0.lock().unwrap().events.is_empty());
}

#[test]
fn partially_failed_link_is_compensated_before_original_mode() {
    let f = setup();
    let _gate = f.hal.begin_write().unwrap();
    let plan = checkpoint(&f.hal, 3).unwrap();
    let error = plan
        .execute_linked(
            &f.hal,
            &mut || Ok(()),
            &mut |_| {
                mark(&f, "PARTIAL_LINK");
                bail!("隔离关联设置部分失败")
            },
            &mut |_| Ok(()),
            &mut || {
                assert_eq!(f.device.0.lock().unwrap().pages[1][1], 3);
                mark(&f, "LINK_ROLLBACK");
                Ok(())
            },
        )
        .unwrap_err();
    assert!(error.to_string().contains("隔离关联设置部分失败"));
    assert!(position(&f, "PARTIAL_LINK") < position(&f, "LINK_ROLLBACK"));
    assert!(position(&f, "LINK_ROLLBACK") < position(&f, "MODE:2"));
    assert_eq!(f.device.0.lock().unwrap().pages[1][1], 2);
}

#[test]
fn a_later_fan_failure_also_compensates_the_completed_link_first() {
    let f = setup();
    let _gate = f.hal.begin_write().unwrap();
    let plan = checkpoint(&f.hal, 3).unwrap();
    f.device.0.lock().unwrap().fail_fan_mode = true;
    assert!(
        plan.execute_linked(
            &f.hal,
            &mut || Ok(()),
            &mut |_| {
                mark(&f, "LINK_COMPLETE");
                Ok(())
            },
            &mut |_| Ok(()),
            &mut || {
                mark(&f, "LINK_ROLLBACK");
                Ok(())
            }
        )
        .is_err()
    );
    assert!(position(&f, "FAN_MODE_FAILURE") < position(&f, "LINK_ROLLBACK"));
    assert!(position(&f, "LINK_ROLLBACK") < position(&f, "MODE:2"));
    assert!(f.hal.recovery().unwrap().oem_mode.is_some());
}

#[test]
fn an_unentered_link_is_neither_executed_nor_compensated() {
    let f = setup();
    let _gate = f.hal.begin_write().unwrap();
    let plan = checkpoint(&f.hal, 3).unwrap();
    assert!(
        plan.execute_linked(
            &f.hal,
            &mut || {
                if f.device
                    .0
                    .lock()
                    .unwrap()
                    .events
                    .iter()
                    .any(|event| event == "MODE:3")
                {
                    bail!("模式命令后撤销");
                }
                Ok(())
            },
            &mut |_| { panic!("撤销后不能进入联动") },
            &mut |_| { panic!("撤销后不能进入节能后的联动") },
            &mut || { panic!("未开始联动不能补偿它") }
        )
        .is_err()
    );
    assert!(position(&f, "MODE:3") < position(&f, "MODE:2"));
}

#[test]
fn link_recovery_failure_does_not_skip_mode_recovery_or_erase_its_baseline() {
    let f = setup();
    let _gate = f.hal.begin_write().unwrap();
    let plan = checkpoint(&f.hal, 3).unwrap();
    let error = plan
        .execute_linked(
            &f.hal,
            &mut || Ok(()),
            &mut |_| bail!("隔离联动写入失败"),
            &mut |_| Ok(()),
            &mut || bail!("隔离联动恢复失败"),
        )
        .unwrap_err();
    let message = format!("{error:#}");
    assert!(message.contains("隔离联动写入失败") && message.contains("隔离联动恢复失败"));
    assert_eq!(f.device.0.lock().unwrap().pages[1][1], 2);
    assert!(f.hal.recovery().unwrap().oem_mode.is_some());
}

#[test]
fn startup_and_resume_reload_the_active_plan_and_skip_the_configured_plan() {
    let f = setup();
    let _gate = f.hal.begin_write().unwrap();
    let old = f.device.0.lock().unwrap().active_plan;
    let plan = checkpoint_for_supply(&f.hal, 3, false).unwrap();
    assert_eq!(plan.target.power_plan, None);
    assert_eq!(plan.target.aspm, Some([2, 1]));
    plan.execute(&f.hal).unwrap();
    assert_eq!(f.device.0.lock().unwrap().active_plan, old);
    assert!(
        f.device
            .0
            .lock()
            .unwrap()
            .events
            .iter()
            .any(|e| e == &format!("PLAN:{old:x}"))
    );
    assert!(
        !f.device
            .0
            .lock()
            .unwrap()
            .events
            .iter()
            .any(|e| e == &format!("PLAN:{:x}", crate::hal::power_plan::SCHEMES[2]))
    );
}

#[test]
fn skipped_plan_does_not_require_an_unused_scheme_or_parse_its_saved_index() {
    let f = setup();
    let _gate = f.hal.begin_write().unwrap();
    {
        let mut state = f.device.0.lock().unwrap();
        state.policy_tables.get_mut(&29).unwrap()[2] = 99;
        state
            .plan_values
            .remove(&crate::hal::power_plan::SCHEMES[2]);
    }
    assert!(checkpoint(&f.hal, 3).is_err());
    let plan = checkpoint_for_supply(&f.hal, 3, false).unwrap();
    plan.execute(&f.hal).unwrap();
    assert!(position(&f, "ASPM") < position(&f, "PAGE4:5:"));
}
