use super::*;
use crate::fanctl::ModePolicy;
#[path = "../../../profiles/apply/fixtures.rs"]
#[allow(dead_code)] // 共用档案夹具，本组不使用GPU控制器。
mod fixtures;
use fixtures::Fixture;
fn setup() -> Fixture {
    let mut f = Fixture::new();
    {
        let mut s = f.device.0.lock().unwrap();
        s.pages[1][1] = 2;
        s.bios[0] = 1;
        s.bios[16] |= 16;
        s.bios[11] |= 16;
    }
    f.hal.set_test_dtt_driver(std::sync::Arc::new(|| Ok(true)));
    f
}
fn request(f: &Fixture, turbo: bool, dtt: bool) -> Request {
    Request {
        expected_revision: revision(&f.hal).unwrap(),
        quiet_fan: None,
        replace_custom_fan: false,
        values: Values {
            turbo_fan: Some(turbo),
            dtt: Some(dtt),
        },
    }
}
#[test]
fn entertainment_saves_preferences_without_sending_the_mode_or_fan_command() {
    let mut f = setup();
    {
        let mut state = f.device.0.lock().unwrap();
        state.pages[1][1] = 3;
        state.pages[1][16] = 3;
        state.pages[4][7] = 20;
        state.pages[4][9] = 1;
        state.pages[4][72] = FAN_MODE_NOISELESS;
        state.bios[11] |= 2;
        state.bios[20] |= 1;
        state.bios[21] |= 1;
    }
    f.hal.set_test_dtt_driver(std::sync::Arc::new(|| Ok(false)));
    f.hal.system_power = std::sync::Arc::new(crate::hal::power_plan::UnavailablePower);
    let _gate = f.hal.begin_write().unwrap();
    apply(&f.hal, &request(&f, true, true)).unwrap();
    {
        let s = f.device.0.lock().unwrap();
        assert_eq!(s.events, ["PAGE4:8:[1]", "PAGE1:32:[1]"]);
        assert_eq!(s.pages[1][1], 3);
        assert_eq!(s.pages[1][16], 3);
        assert_eq!(s.pages[4][7..10], [20, 1, 1]);
        assert_eq!(s.pages[4][72], FAN_MODE_NOISELESS);
    }
    let modes = super::super::read(&f.hal).unwrap();
    let performance = modes
        .targets
        .iter()
        .find(|target| target.mode == 2)
        .unwrap();
    assert!(performance.turbo && performance.dtt);
    assert_eq!(performance.fan.mode(), Some(FAN_MODE_AUTO));
    let error = super::super::checkpoint(&f.hal, 2).err().unwrap();
    assert!(error.to_string().contains("Dynamic Tuning"));
}

#[path = "workflow_tests/entertainment.rs"]
mod entertainment;

#[test]
fn performance_noiseless_compensates_failure_and_restores_fan_and_turbo() {
    let f = setup();
    {
        let mut s = f.device.0.lock().unwrap();
        s.bios[20] |= 1;
        s.bios[21] |= 1;
        s.pages[4][8] = 1;
        s.pages[4][9] = 1;
        s.pages[4][73] = 5;
        s.fan_count = 1;
        s.cpu_temperature = 99;
        s.fail_fan_mode = true;
    }
    let _gate = f.hal.begin_write().unwrap();
    assert!(!fanctl::read_profile(&f.hal).unwrap().custom_supported);
    let mut quiet = request(&f, false, false);
    quiet.quiet_fan = Some(QuietTarget::Current);
    assert!(
        apply(&f.hal, &quiet)
            .unwrap_err()
            .to_string()
            .contains("失败")
    );
    {
        let s = f.device.0.lock().unwrap();
        assert!(s.events.iter().any(|event| event == "FAN_MODE_FAILURE"));
        assert_eq!([s.pages[4][5], s.pages[4][8], s.pages[4][72]], [0, 1, 0]);
    }
    apply(&f.hal, &quiet).unwrap();
    assert!(!f.hal.fan_guard_armed().unwrap());
    let status = read(&f.hal).unwrap().quiet_fan.unwrap();
    assert!(status.active && status.enabled && status.remembered);
    assert_eq!(f.device.0.lock().unwrap().pages[4][72], FAN_MODE_NOISELESS);
    assert!(
        super::super::read(&f.hal)
            .unwrap()
            .targets
            .iter()
            .any(|target| target.mode == 2 && target.fan.mode() == Some(FAN_MODE_NOISELESS))
    );
}

#[test]
fn confirmed_custom_curve_handoff_applies_quiet_in_both_modes() {
    for mode in [2_u8, 3] {
        let f = setup();
        {
            let mut state = f.device.0.lock().unwrap();
            state.pages[1][1] = mode;
            state.bios[20] |= 1;
            state.bios[21] |= 1;
            state.pages[4][5] = FAN_MODE_CUSTOM;
            state.fan_mode = FAN_MODE_CUSTOM;
            state.pages[4][70 + usize::from(mode)] = FAN_MODE_CUSTOM;
            state.pages[4][9] = u8::from(mode == 3);
        }
        let _gate = f.hal.begin_write().unwrap();
        let mut quiet = request(&f, false, false);
        quiet.quiet_fan = Some(QuietTarget::Current);
        assert!(apply(&f.hal, &quiet).is_err());
        assert!(f.device.0.lock().unwrap().events.is_empty());

        quiet.replace_custom_fan = true;
        quiet.values.dtt = Some(true);
        assert!(apply(&f.hal, &quiet).is_err());
        quiet.values.dtt = Some(false);
        assert!(f.device.0.lock().unwrap().events.is_empty());
        f.device.0.lock().unwrap().fail_fan_mode = true;
        assert!(apply(&f.hal, &quiet).is_err());
        assert_eq!(f.device.0.lock().unwrap().pages[4][5], FAN_MODE_CUSTOM);
        apply(&f.hal, &quiet).unwrap();
        assert!(apply(&f.hal, &quiet).is_err());
        let active = read(&f.hal).unwrap();
        assert_eq!(active.current_mode, mode);
        assert!(active.quiet_fan.as_ref().unwrap().active);
        assert_eq!(
            f.device.0.lock().unwrap().fan_command,
            Some(FAN_MODE_NOISELESS)
        );

        assert!(active.quiet_fan.as_ref().unwrap().enabled);
    }
}

#[test]
fn manual_auto_preserves_entertainment_noiseless_preference_for_preset_entry() {
    let f = setup();
    {
        let mut state = f.device.0.lock().unwrap();
        state.bios[20] |= 1;
        state.bios[21] |= 1;
    }
    let _gate = f.hal.begin_write().unwrap();
    let mut performance = request(&f, false, false);
    performance.quiet_fan = Some(QuietTarget::Current);
    apply(&f.hal, &performance).unwrap();

    super::super::checkpoint(&f.hal, 3)
        .unwrap()
        .execute(&f.hal)
        .unwrap();
    assert!(!read(&f.hal).unwrap().quiet_fan.unwrap().enabled);
    let mut entertainment = request(&f, false, false);
    entertainment.quiet_fan = Some(QuietTarget::EntertainmentOn);
    apply(&f.hal, &entertainment).unwrap();
    {
        let state = f.device.0.lock().unwrap();
        assert_eq!((state.pages[4][5], state.pages[4][9]), (0, 1));
        assert_eq!((state.pages[4][72], state.pages[4][73]), (8, 0));
        assert_eq!(state.fan_command, Some(FAN_MODE_NOISELESS));
    }
    let quiet = fanctl::read_profile(&f.hal).unwrap();
    assert_eq!(quiet.current_mode, FAN_MODE_NOISELESS);
    assert_eq!(f.device.0.lock().unwrap().pages[4][5], FAN_MODE_AUTO);
    assert_eq!(quiet.offset_slot, Some(3));

    fanctl::set_mode(&f.hal, FAN_MODE_AUTO).unwrap();
    assert_eq!(
        fanctl::read_profile(&f.hal).unwrap().current_mode,
        FAN_MODE_AUTO
    );
    let quiet = read(&f.hal).unwrap().quiet_fan.unwrap();
    assert!(quiet.enabled && !quiet.active);
    {
        let state = f.device.0.lock().unwrap();
        assert_eq!(state.fan_command, Some(FAN_MODE_AUTO));
        assert_eq!(
            (state.pages[4][5], state.pages[4][9], state.pages[4][73]),
            (0, 1, 0)
        );
    }

    super::super::checkpoint(&f.hal, 2)
        .unwrap()
        .execute(&f.hal)
        .unwrap();
    assert!(read(&f.hal).unwrap().quiet_fan.unwrap().enabled);
    assert_eq!(f.device.0.lock().unwrap().pages[4][9], 1);
    super::super::checkpoint(&f.hal, 3)
        .unwrap()
        .execute(&f.hal)
        .unwrap();
    assert_eq!(
        fanctl::read_profile(&f.hal).unwrap().current_mode,
        FAN_MODE_NOISELESS
    );
}

#[test]
fn entertainment_noiseless_can_switch_off_after_save_failure() {
    let f = setup();
    {
        let mut state = f.device.0.lock().unwrap();
        state.pages[1][1] = 3;
        state.bios[20] |= 1;
        state.pages[4][8] = 1;
        state.pages[4][9] = 1;
        state.fan_mode = FAN_MODE_NOISELESS;
        state.fail_option_save = Some((4, 9));
    }
    let _gate = f.hal.begin_write().unwrap();
    let mut quiet = request(&f, true, false);
    quiet.quiet_fan = Some(QuietTarget::EntertainmentOff);
    assert!(apply(&f.hal, &quiet).is_err());
    {
        let state = f.device.0.lock().unwrap();
        assert_eq!(
            (state.pages[4][5], state.pages[4][8], state.pages[4][9]),
            (0, 1, 1)
        );
        assert_eq!(state.fan_command, Some(FAN_MODE_NOISELESS));
    }
    apply(&f.hal, &quiet).unwrap();
    {
        let state = f.device.0.lock().unwrap();
        assert_eq!(
            (state.pages[4][5], state.pages[4][8], state.pages[4][9]),
            (0, 1, 0)
        );
        assert_eq!(state.fan_command, Some(FAN_MODE_AUTO));
    }
}

#[test]
fn hot_performance_selection_applies_remembered_noiseless_mode_and_offset() {
    let f = setup();
    {
        let mut state = f.device.0.lock().unwrap();
        state.pages[1][1] = 3;
        state.bios[20] |= 1;
        state.bios[21] |= 1;
        state.pages[4][72] = FAN_MODE_NOISELESS;
        state.pages[4][76] = 20;
        state.cpu_temperature = 99;
    }
    let _gate = f.hal.begin_write().unwrap();
    let target = super::super::read(&f.hal)
        .unwrap()
        .targets
        .into_iter()
        .find(|target| target.mode == 2)
        .unwrap();
    assert_eq!(target.fan, ModePolicy::explicit(FAN_MODE_NOISELESS, FAN_MODE_NOISELESS, Some(20)));
    super::super::apply_preview(&f.hal, &target).unwrap();
    assert!(!f.hal.fan_guard_armed().unwrap());
    let state = f.device.0.lock().unwrap();
    assert_eq!(state.pages[1][1], 2);
    assert_eq!(state.pages[4][5], FAN_MODE_NOISELESS);
    assert_eq!(state.pages[4][7], 20);
}

#[test]
fn real_workflow_sends_independent_dtt_without_touching_windows_or_fan() {
    let f = setup();
    let _gate = f.hal.begin_write().unwrap();
    apply(&f.hal, &request(&f, false, true)).unwrap();
    {
        let s = f.device.0.lock().unwrap();
        assert_eq!(s.pages[1][32], 1);
        assert_eq!(s.pages[4][8], 0);
        assert!(s.events.iter().any(|e| e == "MODE:130"));
        assert!(
            !s.events.iter().any(|e| e.starts_with("PAGE4:5:")
                || e.starts_with("ASPM")
                || e.starts_with("PLAN"))
        );
    }
    assert!(f.hal.recovery().unwrap().oem_mode.is_some());
}
#[test]
fn real_turbo_fan_failure_restores_saved_flags_and_keeps_initial_retry() {
    let f = setup();
    let _gate = f.hal.begin_write().unwrap();
    f.device.0.lock().unwrap().fail_fan_mode = true;
    let error = apply(&f.hal, &request(&f, true, true))
        .unwrap_err()
        .to_string();
    assert!(error.contains("失败") || error.contains("SetDCHU"));
    {
        let s = f.device.0.lock().unwrap();
        assert_eq!(s.pages[4][8], 0);
        assert_eq!(s.pages[1][32], 0);
        assert_eq!(s.pages[4][5], 0);
        assert!(s.events.iter().any(|e| e == "FAN_MODE_FAILURE"));
    }
    assert!(f.hal.recovery().unwrap().oem_mode.is_some());
}
#[test]
fn command_and_compensation_failure_keep_shared_baseline_for_user_retry() {
    let f = setup();
    let _gate = f.hal.begin_write().unwrap();
    f.device.0.lock().unwrap().fail_mode = true;
    let error = apply(&f.hal, &request(&f, true, true))
        .unwrap_err()
        .to_string();
    assert!(error.contains("恢复失败"));
    assert!(f.hal.recovery().unwrap().oem_mode.is_some());
    f.device.0.lock().unwrap().fail_mode = false;
    super::super::checkpoint(&f.hal, 2)
        .unwrap()
        .execute(&f.hal)
        .unwrap();
    assert_eq!(f.device.0.lock().unwrap().pages[1][1], 2);
}
#[test]
fn revoked_before_first_mode_output_does_not_write_or_create_baseline() {
    let f = setup();
    let _gate = f.hal.begin_write().unwrap();
    let plan = super::super::checkpoint(&f.hal, 3).unwrap();
    let mut checks = 0;
    let result = plan.execute_guarded(&f.hal, &mut || {
        checks += 1;
        if checks == 3 {
            bail!("隔离许可撤销");
        }
        Ok(())
    });
    assert!(result.is_err());
    assert!(f.device.0.lock().unwrap().events.is_empty());
    assert!(f.hal.recovery().unwrap().oem_mode.is_none());
}

#[test]
fn revoked_after_mode_command_compensates_the_original_state() {
    let f = setup();
    let _gate = f.hal.begin_write().unwrap();
    let plan = super::super::checkpoint(&f.hal, 3).unwrap();
    let result = plan.execute_guarded(&f.hal, &mut || {
        if f.device
            .0
            .lock()
            .unwrap()
            .events
            .iter()
            .any(|event| event == "MODE:3")
        {
            bail!("隔离许可撤销");
        }
        Ok(())
    });
    assert!(result.is_err());
    let data = f.device.0.lock().unwrap();
    assert!(data.events.iter().any(|event| event == "MODE:3"));
    assert!(data.events.iter().any(|event| event == "MODE:2"));
    assert_eq!(data.pages[1][1], 2);
}

#[test]
fn revoked_after_ac_preference_save_restores_it_before_sending_any_target_mode() {
    let f = setup();
    let _gate = f.hal.begin_write().unwrap();
    let plan = super::super::checkpoint(&f.hal, 3).unwrap();
    let result = plan.execute_guarded(&f.hal, &mut || {
        if f.device.0.lock().unwrap().pages[1][16] == 3 {
            bail!("隔离偏好保存后撤销");
        }
        Ok(())
    });
    assert!(result.is_err());
    let data = f.device.0.lock().unwrap();
    assert_eq!(data.pages[1][16], 0);
    assert_eq!(data.pages[1][1], 2);
    assert!(!data.events.iter().any(|event| event == "MODE:3"));
    assert!(f.hal.recovery().unwrap().oem_mode.is_some());
}
