mod fixtures;
mod locking;
mod recovery;
use super::*;
use fixtures::*;

#[test]
fn readonly_preview_keeps_actual_and_saved_values_separate_and_never_captures_initial() {
    let (controller, hal, memory) = setup();
    let profile = controller.read(&hal).unwrap();
    let current = profile.current.unwrap();
    assert_eq!(current.reading.current, 80);
    assert_eq!(current.levels, [75, 65]);
    assert_eq!(current.targets, [40; 11]);
    assert!(profile.initial.is_empty());
    assert!(profile.restore_revision.is_none());
    assert!(memory.0.lock().unwrap().writes.is_empty());
    no_native(&memory);
}

#[test]
fn ac_and_already_darker_reports_never_wait_write_or_create_baselines() {
    for (on_ac, current) in [(true, 80), (false, 40), (false, 30)] {
        let (controller, hal, memory) = setup();
        memory.0.lock().unwrap().snapshot.reading.on_ac = on_ac;
        memory.0.lock().unwrap().snapshot.reading.current = current;
        let id = identity(&controller, &hal);
        let result = controller
            .run(&hal, &id, 78, || Ok(true), || panic!("no wait needed"))
            .unwrap()
            .unwrap();
        assert!(result.detail.contains("未修改计划"));
        assert!(controller.read(&hal).unwrap().initial.is_empty());
        assert!(memory.0.lock().unwrap().writes.is_empty());
    }
}

#[test]
fn dc_updates_both_fields_then_checks_actual_report_and_initial_restore_is_shared() {
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
        .unwrap()
        .unwrap();
    assert_eq!(
        memory.0.lock().unwrap().writes,
        [(1, true, 40), (1, false, 40)]
    );
    let profile = controller.read(&hal).unwrap();
    assert_eq!(profile.initial[0].original, [75, 65]);
    assert_eq!(profile.initial[0].owned, [true; 2]);
    controller
        .restore(&hal, profile.restore_revision.as_deref().unwrap())
        .unwrap();
    assert_eq!(memory.0.lock().unwrap().plans[&1], [75, 65]);
    no_native(&memory);
}

#[test]
fn actual_report_not_reaching_target_bounds_writes_and_restores_operation_prestate() {
    let (controller, hal, memory) = setup();
    let id = identity(&controller, &hal);
    let error = controller
        .run(&hal, &id, 48, || Ok(true), || Ok(true))
        .unwrap_err();
    assert!(format!("{error:#}").contains("停止重复写入"));
    let data = memory.0.lock().unwrap();
    assert_eq!(data.writes.len(), 8);
    assert_eq!(data.plans[&1], [75, 65]);
    assert!(controller.scope().unwrap().initial.contains_key(&1));
    drop(data);
    no_native(&memory);
}

#[test]
fn stop_before_start_or_while_waiting_returns_no_message_and_preserves_fields() {
    for cancel_at_start in [true, false] {
        let (controller, hal, memory) = setup();
        let id = identity(&controller, &hal);
        let result = controller
            .run(&hal, &id, 78, || Ok(!cancel_at_start), || Ok(false))
            .unwrap();
        assert!(result.is_none());
        assert!(memory.0.lock().unwrap().writes.is_empty());
        assert!(controller.read(&hal).unwrap().initial.is_empty());
    }
}

#[test]
fn cancellation_immediately_before_the_first_field_does_not_capture_or_refresh_any_plan() {
    let (_, _, memory) = setup();
    let snapshot = transaction::Io::snapshot(memory.as_ref()).unwrap();
    let mut job = model::Job::new(&snapshot, &snapshot.identity().unwrap(), 78).unwrap();
    let mut state = transaction::State::default();
    let mut checks = 0;
    assert!(
        state
            .step(memory.as_ref(), &mut job, true, &mut || {
                checks += 1;
                if checks == 2 {
                    anyhow::bail!("取消");
                }
                Ok(())
            })
            .is_err()
    );
    state.rollback(memory.as_ref(), &job).unwrap();
    assert!(memory.0.lock().unwrap().writes.is_empty());
    assert!(memory.0.lock().unwrap().activations.is_empty());
    assert!(state.initial.is_empty());
}

#[test]
fn stopping_between_fields_or_after_one_pair_compensates_only_attempted_fields() {
    for between_fields in [true, false] {
        let (controller, hal, memory) = setup();
        let id = identity(&controller, &hal);
        let mut waits = 0;
        let result = controller
            .run(
                &hal,
                &id,
                78,
                || Ok(!between_fields || memory.0.lock().unwrap().writes.is_empty()),
                || {
                    waits += 1;
                    Ok(waits == 1)
                },
            )
            .unwrap();
        assert!(result.is_none());
        let data = memory.0.lock().unwrap();
        assert_eq!(data.plans[&1], [75, 65]);
        assert_eq!(data.writes.len(), if between_fields { 2 } else { 4 });
    }
}

#[test]
fn each_partial_write_and_activation_failure_restores_exact_two_field_prestate() {
    for failure in 0..3 {
        let (controller, hal, memory) = setup();
        match failure {
            0 => memory.0.lock().unwrap().fail_writes.push(1),
            1 => memory.0.lock().unwrap().fail_writes.push(2),
            _ => memory.0.lock().unwrap().fail_activations.push(1),
        }
        let id = identity(&controller, &hal);
        assert!(
            controller
                .run(&hal, &id, 78, || Ok(true), || Ok(true))
                .is_err()
        );
        assert_eq!(memory.0.lock().unwrap().plans[&1], [75, 65]);
        no_native(&memory);
    }
}

#[test]
fn ignored_writes_fail_readback_and_restore_only_owned_fields() {
    let (controller, hal, memory) = setup();
    memory.0.lock().unwrap().ignore_writes.push(1);
    let id = identity(&controller, &hal);
    assert!(
        controller
            .run(&hal, &id, 78, || Ok(true), || Ok(true))
            .is_err()
    );
    memory.0.lock().unwrap().plans.get_mut(&1).unwrap()[1] = 99;
    let profile = controller.read(&hal).unwrap();
    assert_eq!(profile.initial[0].owned, [true, false]);
    controller
        .restore(&hal, profile.restore_revision.as_deref().unwrap())
        .unwrap();
    assert_eq!(memory.0.lock().unwrap().plans[&1], [75, 99]);
}

#[test]
fn stale_identity_unknown_event_and_unavailable_report_never_write() {
    let (controller, hal, memory) = setup();
    let id = identity(&controller, &hal);
    assert!(
        controller
            .run(&hal, "stale", 78, || Ok(true), || Ok(true))
            .is_err()
    );
    assert!(
        controller
            .run(&hal, &id, 77, || Ok(true), || Ok(true))
            .is_err()
    );
    memory.0.lock().unwrap().fail_panel = true;
    let preview = controller.read(&hal).unwrap();
    assert!(preview.identity.is_none() && preview.current.is_none());
    assert!(preview.error.unwrap().contains("面板"));
    assert!(memory.0.lock().unwrap().writes.is_empty());
}

#[test]
fn target_table_or_plan_changes_stop_output_and_restore_old_plan_without_switching_back() {
    for change_plan in [false, true] {
        let (controller, hal, memory) = setup();
        let id = identity(&controller, &hal);
        let mut waits = 0;
        let result = controller.run(
            &hal,
            &id,
            78,
            || Ok(true),
            || {
                waits += 1;
                if waits == 2 {
                    let mut data = memory.0.lock().unwrap();
                    if change_plan {
                        data.snapshot.plan = 2;
                    } else {
                        data.snapshot.targets.0[0] = 30;
                    }
                }
                Ok(true)
            },
        );
        assert!(result.is_err());
        let data = memory.0.lock().unwrap();
        assert_eq!(data.plans[&1], [75, 65]);
        if change_plan {
            assert_eq!(data.snapshot.plan, 2);
            assert_eq!(data.activations, [1]);
        }
    }
}

#[test]
fn panel_change_blocks_compensation_and_retains_a_retryable_initial_record() {
    let (controller, hal, memory) = setup();
    let id = identity(&controller, &hal);
    let mut waits = 0;
    let error = controller
        .run(
            &hal,
            &id,
            78,
            || Ok(true),
            || {
                waits += 1;
                if waits == 2 {
                    memory.0.lock().unwrap().snapshot.reading.panel = "replacement".into();
                }
                Ok(true)
            },
        )
        .unwrap_err();
    assert!(format!("{error:#}").contains("本次恢复失败"));
    let preview = controller.read(&hal).unwrap();
    assert!(preview.identity.is_none() && preview.restore_revision.is_none());
    assert!(!preview.initial.is_empty());
    memory.0.lock().unwrap().snapshot.reading.panel = "isolated-panel".into();
    let preview = controller.read(&hal).unwrap();
    controller
        .restore(&hal, preview.restore_revision.as_deref().unwrap())
        .unwrap();
    assert_eq!(memory.0.lock().unwrap().plans[&1], [75, 65]);
}

#[test]
fn dc_to_ac_stops_further_output_after_the_completed_pair() {
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
                    memory.0.lock().unwrap().snapshot.reading.on_ac = true;
                }
                Ok(true)
            },
        )
        .unwrap()
        .unwrap();
    assert_eq!(memory.0.lock().unwrap().writes.len(), 2);
    assert_eq!(memory.0.lock().unwrap().plans[&1], [40; 2]);
}

#[test]
fn failed_target_read_does_not_hide_known_initial_restore_or_require_dchu_to_restore() {
    let (controller, hal, memory) = setup();
    memory.0.lock().unwrap().fail_writes.push(1);
    let id = identity(&controller, &hal);
    assert!(
        controller
            .run(&hal, &id, 78, || Ok(true), || Ok(true))
            .is_err()
    );
    memory.0.lock().unwrap().fail_packet = true;
    let preview = controller.read(&hal).unwrap();
    assert!(preview.identity.is_none() && preview.current.is_none());
    assert!(preview.restore_revision.is_some());
    controller
        .restore(&hal, preview.restore_revision.as_deref().unwrap())
        .unwrap();
    assert_eq!(memory.0.lock().unwrap().plans[&1], [75, 65]);
}

#[test]
fn restore_preview_binds_current_fields_and_failed_compensation_keeps_both_errors() {
    let (controller, hal, memory) = setup();
    memory.0.lock().unwrap().fail_writes = vec![1, 2];
    let id = identity(&controller, &hal);
    let error = controller
        .run(&hal, &id, 78, || Ok(true), || Ok(true))
        .unwrap_err();
    let text = format!("{error:#}");
    assert!(text.contains("失败1") && text.contains("失败2"));
    let preview = controller.read(&hal).unwrap();
    memory.0.lock().unwrap().plans.get_mut(&1).unwrap()[0] = 90;
    let writes = memory.0.lock().unwrap().writes.len();
    assert!(
        controller
            .restore(&hal, preview.restore_revision.as_deref().unwrap())
            .is_err()
    );
    assert_eq!(memory.0.lock().unwrap().writes.len(), writes);
    memory.0.lock().unwrap().fail_writes.clear();
    let preview = controller.read(&hal).unwrap();
    controller
        .restore(&hal, preview.restore_revision.as_deref().unwrap())
        .unwrap();
}
