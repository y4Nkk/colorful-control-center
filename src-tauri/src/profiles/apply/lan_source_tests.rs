use super::*;

#[test]
fn authorization_cleanup_failure_is_reported_after_confirmed_targets_and_keeps_initial_recovery() {
    let f = setup();
    let settings = wanted(&f);
    f.lan.io.fail_finish.store(true, Ordering::SeqCst);
    let steps = apply(&f, &settings);
    assert!(
        steps
            .iter()
            .any(|step| step.step == "网络驱动策略" && step.ok)
    );
    assert!(
        steps
            .iter()
            .any(|step| step.step == "网络授权收尾" && !step.ok)
    );
    assert_eq!(
        f.lan.io.frame.lock().unwrap().devices[0].power_saving(),
        Some(true)
    );
    assert_eq!(
        initial(&f).devices[0].fields[&Field::PowerSavingMode],
        Some(Value::string("0"))
    );
}

#[test]
fn authorization_changes_to_driver_capability_fields_or_record_block_all_profile_outputs() {
    for change in 0..4 {
        let f = setup();
        let mut settings = wanted(&f);
        settings.oem_power_mode = Some(2);
        let memory = f.lan.io.clone();
        let path = f.lan.path();
        *f.lan.io.on_authorize.lock().unwrap() = Some(Box::new(move || {
            if change == 3 {
                store::write(
                    &path,
                    &store::Record::default(),
                    &store::Record {
                        last_error: Some("外部记录".into()),
                        ..Default::default()
                    },
                )
                .unwrap();
            } else {
                let mut frame = memory.frame.lock().unwrap();
                match change {
                    0 => frame.devices[0].identity.driver_version = Some(Value::string("2.0")),
                    1 => frame.capabilities.power_saving = false,
                    _ => {
                        frame.devices[0]
                            .values
                            .insert(Field::WakeOnLink, Some(Value::string("external")));
                    }
                }
            }
        }));
        assert!(apply_settings(&f.hal, &f.gpu, f.devices(), &settings).is_err());
        assert_eq!(f.lan.io.executions.load(Ordering::SeqCst), 0);
        assert_eq!(f.lan.io.cancellations.load(Ordering::SeqCst), 1);
        assert!(f.device.0.lock().unwrap().events.is_empty());
    }
}
#[test]
fn a_changed_recommendation_does_not_invalidate_an_explicit_target_or_mode_sequence() {
    let f = setup();
    let mut settings = wanted(&f);
    settings.oem_power_mode = Some(2);
    let memory = f.lan.io.clone();
    *f.lan.io.on_authorize.lock().unwrap() = Some(Box::new(move || {
        memory.frame.lock().unwrap().recommendation = Some(false);
    }));
    let steps = apply(&f, &settings);
    assert!(steps.iter().all(|step| step.ok));
    assert!(
        steps
            .iter()
            .position(|step| step.step == "OEM 电源模式与联动")
            .unwrap()
            < steps
                .iter()
                .position(|step| step.step == "网络驱动策略")
                .unwrap()
    );
}
#[test]
fn noop_that_changes_after_preparation_cannot_start_an_unprepared_authorization() {
    let f = setup();
    let target = captured(&f);
    let mut scope = f.lan.controller.scope().unwrap();
    let plan = {
        let _gate = f.hal.begin_write().unwrap();
        scope.prepare_profile(&f.hal, &target).unwrap()
    };
    assert!(plan.is_none());
    scope.authorize_profile(None).unwrap();
    f.lan.io.frame.lock().unwrap().devices[0]
        .values
        .insert(Field::PowerSavingMode, Some(Value::string("1")));
    let _gate = f.hal.begin_write().unwrap();
    assert!(scope.prepare_profile(&f.hal, &target).is_err());
    assert_eq!(f.lan.io.authorizations.load(Ordering::SeqCst), 0);
}
#[test]
fn whole_profile_validation_precedes_authorization_and_calibration_ownership_is_respected() {
    let mut f = setup();
    let mut settings = wanted(&f);
    settings.pl1_w = Some(200.0);
    assert!(apply_settings(&f.hal, &f.gpu, f.devices(), &settings).is_err());
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("calibration.json");
    std::fs::write(&path, b"{broken").unwrap();
    f.hal.bind_calibration_record(path);
    settings.pl1_w = None;
    assert!(apply_settings(&f.hal, &f.gpu, f.devices(), &settings).is_err());
    assert_eq!(f.lan.io.authorizations.load(Ordering::SeqCst), 0);
    assert!(!f.lan.path().exists());
}
#[test]
fn unknown_or_inapplicable_target_fields_never_pass_the_current_contract() {
    let f = setup();
    let original = wanted(&f);
    let mut cases = Vec::new();
    for field in [Field::WakeOnSlot, Field::ShutdownWake] {
        let mut settings = original.clone();
        settings.lan_policy.as_mut().unwrap().devices[0]
            .values
            .insert(field, None);
        cases.push(settings);
    }
    let mut invalid = original.clone();
    invalid.lan_policy.as_mut().unwrap().devices[0]
        .values
        .insert(Field::PowerSavingMode, Some(Value::dword(1)));
    cases.push(invalid);
    let mut invalid = original.clone();
    invalid.lan_policy.as_mut().unwrap().devices.clear();
    cases.push(invalid);
    let mut invalid = original.clone();
    let target = invalid.lan_policy.as_mut().unwrap();
    target.devices.push(target.devices[0].clone());
    cases.push(invalid);
    for settings in cases {
        assert!(apply_settings(&f.hal, &f.gpu, f.devices(), &settings).is_err());
    }
    let mut json = serde_json::to_value(original).unwrap();
    json["lanPolicy"]["compatibility"] = true.into();
    assert!(serde_json::from_value::<ProfileSettings>(json).is_err());
    assert_eq!(f.lan.io.authorizations.load(Ordering::SeqCst), 0);
}
#[test]
fn unknown_power_values_refuse_capture_while_nonapplicable_and_unsupported_devices_are_omitted() {
    let f = setup();
    f.lan.io.frame.lock().unwrap().devices[0]
        .values
        .insert(Field::PowerSavingMode, Some(Value::dword(1)));
    assert!(crate::profiles::capture_current(&f.hal, &f.gpu, f.devices(), &crate::profiles::ProfileGroup::ALL).is_err());
    f.lan.io.frame.lock().unwrap().capabilities = Capabilities::default();
    assert!(
        crate::profiles::capture_current(&f.hal, &f.gpu, f.devices(), &crate::profiles::ProfileGroup::ALL)
            .unwrap()
            .lan_policy
            .is_none()
    );
    assert_eq!(f.lan.io.authorizations.load(Ordering::SeqCst), 0);
}
#[test]
fn profile_file_changed_during_authorization_is_rejected_before_outputs() {
    let f = setup();
    let settings = wanted(&f);
    let directory = tempfile::tempdir().unwrap();
    crate::profiles::save_in(directory.path(), settings.clone(), "网络", None).unwrap();
    let file = crate::profiles::load_from(directory.path(), "网络").unwrap();
    let revision = crate::profiles::authorization::digest(&file).unwrap();
    let path = directory.path().to_path_buf();
    let mut changed = settings.clone();
    changed.lan_policy.as_mut().unwrap().devices[0]
        .values
        .insert(Field::PowerSavingMode, Some(Value::string("0")));
    *f.lan.io.on_authorize.lock().unwrap() = Some(Box::new(move || {
        crate::profiles::save_in(&path, changed, "网络", Some(&revision)).unwrap();
    }));
    assert!(
        transact(
            &f.hal,
            &f.gpu,
            f.devices(),
            &settings,
            ModeReview::Confirmed {
                permission: None,
                mode: None,
                file: Some((directory.path(), &file)),
                display: None,
                gpu_power: None,
                graphics_mode: None,
            }
        )
        .is_err()
    );
    assert_eq!(f.lan.io.executions.load(Ordering::SeqCst), 0);
    assert!(!f.lan.path().exists());
}
#[test]
fn concurrent_single_action_waits_for_the_profile_and_cannot_replace_the_initial_record() {
    let f = setup();
    let settings = wanted(&f);
    let current = f.lan.controller.read(&f.hal).unwrap().unwrap();
    let request = Request {
        expected_revision: current.revision,
        device_id: current.frame.devices[0].identity.instance.clone(),
        power_saving: Some(false),
        apply_wake_patch: false,
    };
    let barrier = Arc::new(std::sync::Barrier::new(2));
    let wait = barrier.clone();
    *f.lan.io.on_authorize.lock().unwrap() = Some(Box::new(move || {
        wait.wait();
        wait.wait();
    }));
    std::thread::scope(|scope| {
        let profile = scope.spawn(|| apply_settings(&f.hal, &f.gpu, f.devices(), &settings));
        barrier.wait();
        let (started, receiver) = std::sync::mpsc::channel();
        let controller = &f.lan.controller;
        let hal = &f.hal;
        let single = scope.spawn(move || {
            started.send(()).unwrap();
            controller.apply(hal, &request)
        });
        receiver
            .recv_timeout(std::time::Duration::from_secs(3))
            .unwrap();
        {
            let _gate = f.hal.begin_write().unwrap();
            assert_eq!(f.lan.io.executions.load(Ordering::SeqCst), 0);
        }
        barrier.wait();
        assert!(profile.join().unwrap().unwrap().iter().all(|step| step.ok));
        assert!(single.join().unwrap().is_err());
    });
    assert_eq!(f.lan.io.authorizations.load(Ordering::SeqCst), 1);
    assert_eq!(
        initial(&f).devices[0].fields[&Field::PowerSavingMode],
        Some(Value::string("0"))
    );
}
