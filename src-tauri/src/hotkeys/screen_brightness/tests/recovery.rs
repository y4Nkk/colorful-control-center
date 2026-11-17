use super::*;

#[test]
fn multiple_confirmed_plans_keep_independent_initial_fields_without_reactivating_inactive_plans() {
    let (controller, hal, memory) = setup();
    for plan in [1, 2] {
        {
            let mut data = memory.0.lock().unwrap();
            data.snapshot.plan = plan;
            data.snapshot.reading.current = 80;
        }
        let id = identity(&controller, &hal);
        let mut waits = 0;
        controller
            .run(
                &hal,
                &id,
                78,
                || Ok(true),
                || {
                    waits += 1;
                    if waits == 2 {
                        memory.0.lock().unwrap().snapshot.reading.current = 40;
                    }
                    Ok(true)
                },
            )
            .unwrap();
    }
    let profile = controller.read(&hal).unwrap();
    assert_eq!(profile.initial.len(), 2);
    assert_eq!(profile.initial[0].original, [75, 65]);
    assert_eq!(profile.initial[1].original, [55, 45]);
    controller
        .restore(&hal, profile.restore_revision.as_deref().unwrap())
        .unwrap();
    let data = memory.0.lock().unwrap();
    assert_eq!(data.plans[&1], [75, 65]);
    assert_eq!(data.plans[&2], [55, 45]);
    assert_eq!(data.snapshot.plan, 2);
    assert_eq!(data.activations, [1, 2, 2]);
}

#[test]
fn both_restore_field_errors_are_preserved_and_initial_restore_remains_retryable() {
    let (controller, hal, memory) = setup();
    let id = identity(&controller, &hal);
    let mut waits = 0;
    controller
        .run(
            &hal,
            &id,
            78,
            || Ok(true),
            || {
                waits += 1;
                if waits == 2 {
                    memory.0.lock().unwrap().snapshot.reading.current = 40;
                }
                Ok(true)
            },
        )
        .unwrap();
    memory.0.lock().unwrap().fail_writes = vec![3, 4];
    let profile = controller.read(&hal).unwrap();
    let error = controller
        .restore(&hal, profile.restore_revision.as_deref().unwrap())
        .unwrap_err();
    let diagnostic = format!("{error:#}");
    assert!(diagnostic.contains("插电") && diagnostic.contains("失败3"));
    assert!(diagnostic.contains("电池") && diagnostic.contains("失败4"));
    memory.0.lock().unwrap().fail_writes.clear();
    let profile = controller.read(&hal).unwrap();
    controller
        .restore(&hal, profile.restore_revision.as_deref().unwrap())
        .unwrap();
    assert_eq!(memory.0.lock().unwrap().plans[&1], [75, 65]);
}

#[test]
fn missing_plan_report_blocks_restore_without_fake_values_then_recovers() {
    let (controller, hal, memory) = setup();
    memory.0.lock().unwrap().fail_writes.push(1);
    let id = identity(&controller, &hal);
    assert!(
        controller
            .run(&hal, &id, 78, || Ok(true), || Ok(true))
            .is_err()
    );
    memory.0.lock().unwrap().fail_levels = true;
    let profile = controller.read(&hal).unwrap();
    assert!(profile.initial[0].current.is_none());
    assert!(profile.restore_revision.is_none());
    assert!(profile.restore_error.is_some());
    memory.0.lock().unwrap().fail_levels = false;
    let profile = controller.read(&hal).unwrap();
    controller
        .restore(&hal, profile.restore_revision.as_deref().unwrap())
        .unwrap();
}

#[test]
fn calibration_plan_ownership_rejects_the_first_write_and_no_native_provider_is_used() {
    let (_, _, memory) = setup();
    memory.0.lock().unwrap().writable = false;
    let snapshot = transaction::Io::snapshot(memory.as_ref()).unwrap();
    let mut job = model::Job::new(&snapshot, &snapshot.identity().unwrap(), 78).unwrap();
    let mut state = transaction::State::default();
    assert!(
        state
            .step(memory.as_ref(), &mut job, true, &mut || Ok(()))
            .is_err()
    );
    assert!(state.initial.is_empty());
    assert!(memory.0.lock().unwrap().writes.is_empty());
    no_native(&memory);
}

#[test]
fn missing_or_invalid_injected_panel_report_never_falls_back_to_the_machine() {
    let (controller, mut hal, memory) = setup();
    hal.test_screen_brightness = None;
    let profile = controller.read(&hal).unwrap();
    assert!(profile.error.unwrap().contains("未注入"));
    assert!(profile.identity.is_none());
    let report = memory.clone();
    hal.test_screen_brightness = Some(std::sync::Arc::new(move || {
        transaction::Io::panel(report.as_ref())
    }));
    memory.0.lock().unwrap().snapshot.reading.current = 101;
    let profile = controller.read(&hal).unwrap();
    assert!(profile.error.unwrap().contains("无效"));
    assert!(profile.identity.is_none());
    assert!(memory.0.lock().unwrap().writes.is_empty());
    no_native(&memory);
}

#[test]
fn permission_excludes_transient_readings_but_binds_panel_plan_and_all_targets() {
    let (_, _, memory) = setup();
    let mut snapshot = transaction::Io::snapshot(memory.as_ref()).unwrap();
    let id = snapshot.identity().unwrap();
    snapshot.reading.current = 20;
    snapshot.reading.on_ac = true;
    snapshot.levels = [0, 100];
    assert_eq!(snapshot.identity().unwrap(), id);
    let original = snapshot.clone();
    snapshot.plan = u128::MAX;
    assert_ne!(snapshot.identity().unwrap(), id);
    snapshot = original.clone();
    snapshot.reading.panel = "other".into();
    assert_ne!(snapshot.identity().unwrap(), id);
    snapshot = original;
    snapshot.targets.0[10] = 0;
    assert_ne!(snapshot.identity().unwrap(), id);
}
