use super::*;

#[test]
fn unsupported_models_are_read_only_and_cannot_fall_through_to_generic_mode_writes() {
    let f = Fixture::new();
    enable_policy(&f);
    let before = {
        let mut state = f.device.0.lock().unwrap();
        state.bios[16] |= 16;
        state.events.clone()
    };
    let _gate = f.hal.begin_write().unwrap();
    for low in [0xf0, 0xf4] {
        f.device.0.lock().unwrap().pages[0][2..6].copy_from_slice(&[0x50, low, 0, 0]);
        let profile = powerctl::mode::read(&f.hal).unwrap();
        assert!(profile.available_modes.is_empty() && profile.targets.is_empty());
        assert!(profile.blocked_reason.unwrap().contains("尚未支持"));
        let error = powerctl::mode::checkpoint(&f.hal, 2).err().unwrap();
        assert!(error.to_string().contains("尚未支持"));
        assert!(
            powerctl::mode::boost::capture_values(&f.hal)
                .unwrap()
                .is_none()
        );
        let options = powerctl::mode::boost::read(&f.hal).unwrap();
        assert!(options.blocked_reason.unwrap().contains("尚未支持"));
        for enabled in [false, true] {
            let request = powerctl::mode::boost::Request {
                expected_revision: options.revision.clone(),
                quiet_fan: None,
                replace_custom_fan: false,
                values: powerctl::mode::boost::Values {
                    turbo_fan: Some(enabled),
                    dtt: None,
                },
            };
            assert!(
                powerctl::mode::boost::apply(&f.hal, &request)
                    .unwrap_err()
                    .to_string()
                    .contains("尚未支持")
            );
        }
    }
    assert_eq!(f.device.0.lock().unwrap().events, before);
    assert!(f.hal.recovery().unwrap().oem_mode.is_none());
}

#[test]
fn unreadable_plan_blocks_restore_and_retains_other_current_values() {
    let f = Fixture::new();
    enable_policy(&f);
    let _gate = f.hal.begin_write().unwrap();
    for mode in [4, 2] {
        powerctl::mode::checkpoint(&f.hal, mode)
            .unwrap()
            .execute(&f.hal)
            .unwrap();
    }
    f.device.0.lock().unwrap().plan_values.remove(&SCHEMES[2]);
    let context = f.devices().mode_context(&f.hal, &f.gpu).unwrap();
    let profile = powerctl::mode::selection::read(&context).unwrap();
    assert!(!profile.can_restore);
    assert!(
        profile
            .restore_blocked_reason
            .unwrap()
            .contains("系统计划恢复前态不可读")
    );
    let plans = profile.restore.unwrap().system_plans;
    assert_eq!(plans.len(), 2);
    assert_eq!(
        plans.iter().filter(|plan| plan.current.is_some()).count(),
        1
    );
    assert_eq!(
        plans.iter().filter(|plan| plan.current.is_none()).count(),
        1
    );
    assert!(f.hal.recovery().unwrap().oem_mode.is_some());
}

#[test]
fn repeated_modes_restore_every_touched_plan_to_its_first_value() {
    let f = Fixture::new();
    enable_policy(&f);
    let initial = f.device.0.lock().unwrap().plan_values.clone();
    let _gate = f.hal.begin_write().unwrap();
    for mode in [4, 2, 4] {
        powerctl::mode::checkpoint(&f.hal, mode)
            .unwrap()
            .execute(&f.hal)
            .unwrap();
    }
    let mut target = powerctl::mode::read(&f.hal).unwrap().restore.unwrap();
    assert_eq!(target.system_plans.len(), 3);
    let revision = target.revision.clone();
    target.bind_system(f.hal.system_power.as_ref()).unwrap();
    assert!(
        target
            .system_plans
            .iter()
            .all(|plan| plan.current.is_some())
    );
    let bound = target.revision;
    f.device
        .0
        .lock()
        .unwrap()
        .plan_values
        .get_mut(&SCHEMES[1])
        .unwrap()[0] = 2;
    let mut reread = powerctl::mode::read(&f.hal).unwrap().restore.unwrap();
    reread.bind_system(f.hal.system_power.as_ref()).unwrap();
    assert_ne!(reread.revision, bound);
    powerctl::mode::restore(&f.hal, &revision).unwrap();
    let state = f.device.0.lock().unwrap();
    assert_eq!(state.active_plan, SCHEMES[0]);
    assert_eq!(state.plan_values, initial);
}

#[test]
fn public_restore_rejects_a_changed_secondary_plan_before_any_output_then_retries() {
    let f = Fixture::new();
    enable_policy(&f);
    let initial = f.device.0.lock().unwrap().plan_values.clone();
    let revision = {
        let _gate = f.hal.begin_write().unwrap();
        for mode in [4, 2] {
            powerctl::mode::checkpoint(&f.hal, mode)
                .unwrap()
                .execute(&f.hal)
                .unwrap();
        }
        let context = f.devices().mode_context(&f.hal, &f.gpu).unwrap();
        let profile = powerctl::mode::selection::read(&context).unwrap();
        assert!(
            profile.restore_blocked_reason.is_none(),
            "{:?}",
            profile.restore_blocked_reason
        );
        profile.restore.unwrap().revision
    };
    f.device
        .0
        .lock()
        .unwrap()
        .plan_values
        .get_mut(&SCHEMES[2])
        .unwrap()[1] = 2;
    let before = f.device.0.lock().unwrap().events.clone();
    assert!(
        powerctl::mode::restoration::restore(
            f.devices().mode_context(&f.hal, &f.gpu).unwrap(),
            &revision,
            &mut || Ok(())
        )
        .is_err()
    );
    assert_eq!(f.device.0.lock().unwrap().events, before);
    let next = {
        let _gate = f.hal.begin_write().unwrap();
        let context = f.devices().mode_context(&f.hal, &f.gpu).unwrap();
        powerctl::mode::selection::read(&context)
            .unwrap()
            .restore
            .unwrap()
            .revision
    };
    powerctl::mode::restoration::restore(
        f.devices().mode_context(&f.hal, &f.gpu).unwrap(),
        &next,
        &mut || Ok(()),
    )
    .unwrap();
    assert_eq!(f.device.0.lock().unwrap().plan_values, initial);
    assert!(f.hal.recovery().unwrap().oem_mode.is_none());
}
