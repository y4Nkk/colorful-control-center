use super::*;

#[test]
fn isolated_driver_query_is_explicit_and_never_falls_back_to_real_devices() {
    let f = Fixture::new();
    {
        let mut state = f.device.0.lock().unwrap();
        state.pages[1][1] = 2;
        state.bios[0] = 1;
        state.bios[11] = 16;
        state.bios[16] = 16;
    }
    let profile = boost::read(&f.hal).unwrap();
    assert_eq!(profile.dtt_driver_available, None);
    assert!(
        profile
            .warnings
            .iter()
            .any(|warning| warning.contains("未注入"))
    );
    assert!(apply_settings(&f.hal, &f.gpu, f.devices(), &wanted(false, true)).is_err());
    assert!(f.device.0.lock().unwrap().events.is_empty());
}

#[test]
fn cached_dtt_with_missing_driver_blocks_plain_mode_before_mutation() {
    let mut f = setup();
    f.hal.set_test_dtt_driver(std::sync::Arc::new(|| Ok(false)));
    {
        let mut state = f.device.0.lock().unwrap();
        state.pages[1][1] = 3;
        state.pages[1][32] = 1;
    }
    let settings = ProfileSettings {
        oem_power_mode: Some(2),
        ..Default::default()
    };
    assert!(apply_settings(&f.hal, &f.gpu, f.devices(), &settings).is_err());
    assert!(f.device.0.lock().unwrap().events.is_empty());
    assert!(f.hal.recovery().unwrap().oem_mode.is_none());
}

#[test]
fn driver_change_after_options_or_mode_preparation_refuses_all_target_outputs() {
    for mode in [false, true] {
        let mut f = setup();
        let available = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        let driver = available.clone();
        f.hal.set_test_dtt_driver(std::sync::Arc::new(move || {
            Ok(driver.load(std::sync::atomic::Ordering::SeqCst))
        }));
        let _gate = f.hal.begin_write().unwrap();
        if mode {
            let plan =
                powerctl::mode::checkpoint_for_profile(&f.hal, 2, Some(&values(false, true)))
                    .unwrap();
            available.store(false, std::sync::atomic::Ordering::SeqCst);
            assert!(plan.execute(&f.hal).is_err());
        } else {
            let plan = boost::Plan::prepare(&f.hal, &values(false, true)).unwrap();
            available.store(false, std::sync::atomic::Ordering::SeqCst);
            assert!(
                plan.execute(&f.hal, false, &mut boost::Change::default())
                    .is_err()
            );
        }
        assert!(f.device.0.lock().unwrap().events.is_empty());
        assert!(f.hal.recovery().unwrap().oem_mode.is_none());
    }
}

#[test]
fn option_noop_never_queries_unavailable_power_plan_or_creates_initial_state() {
    let mut f = setup();
    f.hal.system_power = std::sync::Arc::new(crate::hal::power_plan::UnavailablePower);
    assert!(apply(&f, &wanted(false, false)).iter().all(|step| step.ok));
    assert!(f.hal.recovery().unwrap().oem_mode.is_none());
    assert!(f.device.0.lock().unwrap().events.is_empty());
}

#[test]
fn changed_context_cannot_be_captured_as_an_old_initial_restore_state() {
    for mode in [false, true] {
        let f = setup();
        super::super::mode_tests::enable_policy(&f);
        f.device.0.lock().unwrap().bios[16] |= 16;
        let _gate = f.hal.begin_write().unwrap();
        if mode {
            let plan =
                powerctl::mode::checkpoint_for_profile(&f.hal, 2, Some(&values(false, true)))
                    .unwrap();
            f.device.0.lock().unwrap().active_plan = crate::hal::power_plan::SCHEMES[1];
            assert!(plan.execute(&f.hal).is_err());
        } else {
            let plan = boost::Plan::prepare(&f.hal, &values(false, true)).unwrap();
            f.device.0.lock().unwrap().active_plan = crate::hal::power_plan::SCHEMES[1];
            assert!(
                plan.execute(&f.hal, false, &mut boost::Change::default())
                    .is_err()
            );
        }
        assert!(f.device.0.lock().unwrap().events.is_empty());
    }
}

#[test]
fn an_old_uncombined_mode_preview_does_not_authorize_new_option_bits() {
    let f = setup();
    let old = {
        let _gate = f.hal.begin_write().unwrap();
        powerctl::mode::checkpoint(&f.hal, 2).unwrap().target
    };
    let mut settings = wanted(true, true);
    settings.oem_power_mode = Some(2);
    let file = crate::profiles::ProfileFile {
        version: 1,
        name: "组合".into(),
        saved_at_ms: 0,
        settings,
    };
    assert!(
        apply_reviewed(
            &f.hal,
            &f.gpu,
            f.devices(),
            &file,
            &file,
            Some(&old),
            None,
            None
        )
        .is_err()
    );
    assert!(f.device.0.lock().unwrap().events.is_empty());
}

#[test]
fn active_turbo_conflict_is_mode_dependent_and_zero_write_for_software_curves() {
    let f = setup();
    let curves = serde_json::from_value(json!({"cpu":{"points":[{"tempC":40,"dutyPct":30},{"tempC":95,"dutyPct":100}],"hysteresisC":3,"rampPctPerS":10}})).unwrap();
    let mut settings = wanted(true, false);
    settings.oem_power_mode = Some(2);
    settings.fan_software_curves = Some(curves);
    assert!(apply_settings(&f.hal, &f.gpu, f.devices(), &settings).is_err());
    assert!(f.device.0.lock().unwrap().events.is_empty());
    settings.oem_power_mode = Some(3);
    assert!(crate::profiles::validate(&settings).is_ok());
}
