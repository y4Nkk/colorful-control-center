use super::*;

#[test]
fn profile_disabling_turbo_keeps_firmware_fan_policy_without_memory() {
    let f = setup();
    {
        let mut state = f.device.0.lock().unwrap();
        state.bios[20] = 1;
        state.pages[1][1] = 3;
        state.pages[4][8] = 1;
        state.fan_mode = 2;
        state.ignore_fan_write = true;
    }
    let _gate = f.hal.begin_write().unwrap();
    let plan =
        powerctl::mode::checkpoint_for_profile(&f.hal, 2, Some(&values(false, false))).unwrap();
    assert!(!plan.target.turbo);
    assert_eq!(
        plan.target.fan,
        fanctl::ModePolicy::Firmware { saved_mode: 0 }
    );
    plan.execute(&f.hal).unwrap();
    let state = f.device.0.lock().unwrap();
    assert_eq!((state.pages[1][1], state.pages[4][8]), (2, 0));
    assert_eq!((state.fan_mode, state.fan_command), (2, None));
}

#[test]
fn preset_releases_quiet_fan_before_turbo_without_overwriting_source_memory() {
    for fail in [false, true] {
        let f = setup();
        {
            let mut state = f.device.0.lock().unwrap();
            state.bios[20] = 0x41;
            state.pages[1][1] = 3;
            state.pages[4][8] = 1;
            state.pages[4][9] = 1;
            state.pages[4][73] = 8;
            state.fan_mode = 8;
            state.ignore_fan_write = fail;
        }
        let _gate = f.hal.begin_write().unwrap();
        let plan = powerctl::mode::checkpoint(&f.hal, 2).unwrap();
        let result = plan.execute(&f.hal);
        let state = f.device.0.lock().unwrap();
        let turbo = state.events.iter().position(|event| event == "MODE:66");
        assert_eq!((state.pages[4][9], state.pages[4][73]), (1, 8));
        if fail {
            assert!(result.unwrap_err().to_string().contains("EC 风扇档位回读"));
            assert_eq!(turbo, None);
            assert_eq!((state.pages[1][1], state.fan_mode), (3, 8));
        } else {
            result.unwrap();
            let release = state
                .events
                .iter()
                .position(|event| event == "DCHU:121")
                .unwrap();
            assert!(release < turbo.unwrap());
            assert_eq!((state.pages[1][1], state.fan_mode), (2, 0));
        }
    }
}

#[test]
fn firmware_only_fan_stays_visible_in_comparison_but_is_not_replayed() {
    let f = Fixture::new();
    {
        let mut state = f.device.0.lock().unwrap();
        state.bios[20] = 1;
        state.pages[1][1] = 0;
        state.fan_mode = 2;
    }
    let captured = crate::profiles::capture_current(
        &f.hal,
        &f.gpu,
        f.devices(),
        &[crate::profiles::ProfileGroup::Performance],
    )
    .unwrap();
    assert_eq!(captured.oem_power_mode, Some(0));
    assert!(
        captured.fan_mode.is_none()
            && captured.fan_curves.is_none()
            && captured.fan_rpm_curves.is_none()
    );
    let comparison =
        crate::profiles::capture::compare_current(&f.hal, &f.gpu, f.devices(), &captured).unwrap();
    assert_eq!(comparison.fan_mode, Some(2));
    assert!(apply(&f, &captured).iter().all(|step| step.ok));
    let state = f.device.0.lock().unwrap();
    assert_eq!((state.fan_mode, state.fan_command), (2, None));
}

#[test]
fn rollback_after_turbo_handoff_restores_source_fan_and_settings() {
    let f = setup();
    {
        let mut state = f.device.0.lock().unwrap();
        state.bios[20] = 0x41;
        state.pages[1][1] = 3;
        state.pages[4][5] = 8;
        state.pages[4][8] = 1;
        state.pages[4][73] = 8;
        state.fan_mode = 8;
    }
    let _gate = f.hal.begin_write().unwrap();
    let plan = powerctl::mode::checkpoint(&f.hal, 2).unwrap();
    let error = plan
        .execute_guarded(&f.hal, &mut || {
            if f.device.0.lock().unwrap().fan_command == Some(0) {
                anyhow::bail!("隔离测试撤销预设权限");
            }
            Ok(())
        })
        .unwrap_err();
    assert!(error.to_string().contains("已恢复写入前状态"));
    let state = f.device.0.lock().unwrap();
    assert!(!state.events.iter().any(|event| event == "MODE:66"));
    assert_eq!(
        (
            state.pages[1][1],
            state.pages[4][5],
            state.pages[4][73],
            state.fan_mode
        ),
        (3, 8, 8, 8)
    );
}

#[test]
fn restoring_turbo_from_another_preset_releases_quiet_fan_first() {
    let f = setup();
    {
        let mut state = f.device.0.lock().unwrap();
        state.bios[20] = 1;
        state.pages[4][8] = 1;
        state.pages[4][9] = 1;
    }
    let _gate = f.hal.begin_write().unwrap();
    let plan = powerctl::mode::checkpoint(&f.hal, 3).unwrap();
    plan.execute(&f.hal).unwrap();
    {
        let mut state = f.device.0.lock().unwrap();
        assert_eq!(state.fan_mode, 8);
        state.events.clear();
    }
    plan.restore(&f.hal).unwrap();
    let state = f.device.0.lock().unwrap();
    let release = state
        .events
        .iter()
        .position(|event| event == "DCHU:121")
        .unwrap();
    let turbo = state
        .events
        .iter()
        .position(|event| event == "MODE:66")
        .unwrap();
    assert!(release < turbo);
    assert_eq!((state.pages[1][1], state.fan_mode), (2, 0));
}
