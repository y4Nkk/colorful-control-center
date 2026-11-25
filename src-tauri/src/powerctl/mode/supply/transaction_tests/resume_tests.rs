use super::clock_tests::{raw, with_clocks};
use super::*;
use crate::gpuctl::turbo::mode::ClockObservation;
#[cfg(not(control_sim))]
use crate::powerctl::mode::supply::controller::Controller;
use ccc_core::gpu_turbo::mode::SkipReason;

#[test]
fn wake_skips_unavailable_driver_and_policy_without_touching_frequency_or_approval() {
    let (_dir, store, io) = setup();
    with_clocks(&io, true);
    enable(&io, &store);
    transaction::execute(&io, &store, Trigger::SupplyChange, &mut || Ok(())).unwrap();
    let first = store.load().unwrap().unwrap();
    let counts = {
        let data = io.0.borrow();
        (data.clock_reads, data.clock_writes)
    };
    {
        let mut data = io.0.borrow_mut();
        data.driver_error = Some("隔离驱动睡眠后尚未就绪".into());
        data.review.clocks = None;
        data.clocks = None;
    }
    let applied = transaction::execute(&io, &store, Trigger::Resume, &mut || Ok(()))
        .unwrap()
        .unwrap();
    assert_eq!(applied.trigger, Trigger::Resume);
    let record = store.load().unwrap().unwrap();
    assert!(record.enabled && record.attempt.is_none());
    assert_eq!(record.clock_approval, first.clock_approval);
    assert_eq!(
        record.initial.as_ref().unwrap().gpu_clocks,
        first.initial.unwrap().gpu_clocks
    );
    assert_eq!(
        record.last.as_ref().unwrap().gpu_clocks,
        ClockObservation::Skipped(SkipReason::Resume)
    );
    let data = io.0.borrow();
    assert_eq!((data.clock_reads, data.clock_writes), counts);
    assert!(data.clocks.is_none());
}

#[test]
fn a_frequency_reset_after_sleep_is_unknown_until_a_fresh_approved_write() {
    let (_dir, store, io) = setup();
    with_clocks(&io, true);
    enable(&io, &store);
    transaction::execute(&io, &store, Trigger::SupplyChange, &mut || Ok(())).unwrap();
    let baseline = store
        .load()
        .unwrap()
        .unwrap()
        .initial
        .unwrap()
        .gpu_clocks
        .unwrap();
    io.0.borrow_mut().clocks.as_mut().unwrap().offsets[0].core_khz = Some(222_222);
    transaction::execute(&io, &store, Trigger::Resume, &mut || Ok(())).unwrap();
    let count = io.0.borrow().clock_writes;
    assert_eq!(
        io.0.borrow().clocks.as_ref().unwrap().offsets[0].core_khz,
        Some(222_222)
    );
    let view = transaction::read(&io, &store).unwrap();
    assert!(view.error.is_none() && view.can_restore);
    transaction::execute(&io, &store, Trigger::SupplyChange, &mut || Ok(())).unwrap();
    assert_eq!(io.0.borrow().clock_writes, count + 1);
    assert_eq!(
        store
            .load()
            .unwrap()
            .unwrap()
            .initial
            .unwrap()
            .gpu_clocks
            .unwrap(),
        baseline
    );
    let view = transaction::read(&io, &store).unwrap();
    transaction::restore(&io, &store, &view.revision, &mut || Ok(())).unwrap();
    assert_eq!(io.0.borrow().clocks.as_ref().unwrap(), &raw());
}

#[test]
fn skipping_at_wake_does_not_authorize_a_new_policy_or_driver_for_later_writes() {
    for driver in [false, true] {
        let (_dir, store, io) = setup();
        with_clocks(&io, true);
        enable(&io, &store);
        transaction::execute(&io, &store, Trigger::SupplyChange, &mut || Ok(())).unwrap();
        let approval = store.load().unwrap().unwrap().clock_approval;
        if driver {
            io.0.borrow_mut().driver_identity = "e".repeat(64);
        } else {
            io.0.borrow_mut()
                .review
                .clocks
                .as_mut()
                .unwrap()
                .policy
                .saved[0]
                .core_mhz += 1;
        }
        transaction::execute(&io, &store, Trigger::Resume, &mut || Ok(())).unwrap();
        let counts = {
            let data = io.0.borrow();
            (data.writes, data.clock_writes)
        };
        assert!(transaction::execute(&io, &store, Trigger::SupplyChange, &mut || Ok(())).is_err());
        let data = io.0.borrow();
        assert_eq!((data.writes, data.clock_writes), counts);
        assert_eq!(store.load().unwrap().unwrap().clock_approval, approval);
    }
}

#[test]
fn wake_still_rejects_other_owned_domains_and_revocation_before_output() {
    for external in [false, true] {
        let (_dir, store, io) = setup();
        with_clocks(&io, true);
        enable(&io, &store);
        transaction::execute(&io, &store, Trigger::SupplyChange, &mut || Ok(())).unwrap();
        if external {
            io.set_energy(crate::powerctl::mode::energy::State {
                hardware_p_state: false,
                energy_star: Some(2),
            });
        }
        let before = io.0.borrow().writes;
        assert!(
            transaction::execute(&io, &store, Trigger::Resume, &mut || {
                if external {
                    Ok(())
                } else {
                    bail!("隔离许可撤销")
                }
            })
            .is_err()
        );
        assert_eq!(io.0.borrow().writes, before);
    }
}

#[test]
fn an_unknown_post_wake_frequency_cannot_hide_replacement_of_the_baseline_device() {
    let (_dir, store, io) = setup();
    with_clocks(&io, true);
    enable(&io, &store);
    transaction::execute(&io, &store, Trigger::SupplyChange, &mut || Ok(())).unwrap();
    transaction::execute(&io, &store, Trigger::Resume, &mut || Ok(())).unwrap();
    io.0.borrow_mut().clocks.as_mut().unwrap().devices[0].bus += 1;
    let count = io.0.borrow().writes;
    assert!(transaction::execute(&io, &store, Trigger::SupplyChange, &mut || Ok(())).is_err());
    let view = transaction::read(&io, &store).unwrap();
    assert!(!view.can_restore && view.recovery_error.is_some());
    assert!(transaction::restore(&io, &store, &view.revision, &mut || Ok(())).is_err());
    assert_eq!(io.0.borrow().writes, count);
}

#[test]
fn obsolete_record_and_nullable_frequency_shapes_are_rejected_without_rewriting() {
    let (_dir, store, io) = setup();
    with_clocks(&io, true);
    enable(&io, &store);
    transaction::execute(&io, &store, Trigger::SupplyChange, &mut || Ok(())).unwrap();
    let valid = serde_json::to_value(store.load().unwrap().unwrap()).unwrap();
    for case in 0..12 {
        let mut bad = valid.clone();
        match case {
            0 => bad["version"] = 1.into(),
            1 => {
                bad.as_object_mut().unwrap().remove("clockApproval");
            }
            2 => bad["last"]["gpuClocks"] = serde_json::Value::Null,
            3 => {
                bad["clockApproval"]
                    .as_object_mut()
                    .unwrap()
                    .remove("driverIdentity");
            }
            4 => {
                bad["last"]["gpuClocks"] =
                    serde_json::json!({"status":"unavailable","value":"隔离未知"})
            }
            5 => bad["version"] = 2.into(),
            6 => {
                bad["last"]["mode"]
                    .as_object_mut()
                    .unwrap()
                    .remove("energy");
            }
            7 => bad["last"]["energy"] = serde_json::json!({"hardwarePState":false,"energyStar":0}),
            8 => {
                bad["initial"]["energy"] =
                    serde_json::json!({"hardwarePState":false,"energyStar":0})
            }
            9 => {
                bad["initial"]["mode"]
                    .as_object_mut()
                    .unwrap()
                    .remove("energy");
            }
            10 => bad["version"] = 4.into(),
            _ => {
                let target = bad["captureTarget"].as_object_mut().unwrap();
                target.remove("fan");
                target.insert("fanMode".into(), 0.into());
                target.insert("fanSavedMode".into(), 0.into());
                target.insert("fanOffsetPct".into(), 0.into());
            }
        }
        let bytes = serde_json::to_vec(&bad).unwrap();
        std::fs::write(&store.0, &bytes).unwrap();
        assert!(store.load().is_err());
        assert_eq!(std::fs::read(&store.0).unwrap(), bytes);
    }
}

#[test]
fn unreadable_recovery_keeps_known_targets_but_cannot_be_saved_or_executed() {
    let (_dir, store, io) = setup();
    with_clocks(&io, true);
    enable(&io, &store);
    transaction::execute(&io, &store, Trigger::SupplyChange, &mut || Ok(())).unwrap();
    io.0.borrow_mut().driver_error = Some("隔离频率读取失败".into());
    let before = std::fs::read(&store.0).unwrap();
    let view = transaction::read(&io, &store).unwrap();
    assert!(view.enabled && !view.can_restore && view.recovery_error.is_some());
    let recovery = view.recovery.as_ref().unwrap();
    assert!(matches!(
        recovery.current_gpu_clocks,
        Some(ClockObservation::Unavailable(_))
    ));
    assert_eq!(recovery.initial_gpu_clocks.as_ref().unwrap().state, raw());
    let writes = io.0.borrow().writes;
    assert!(transaction::restore(&io, &store, &view.revision, &mut || Ok(())).is_err());
    assert_eq!(io.0.borrow().writes, writes);
    assert_eq!(std::fs::read(&store.0).unwrap(), before);
}

#[test]
fn deferred_driver_identity_is_persisted_before_the_first_frequency_write_and_never_replaced() {
    let (_dir, store, io) = setup();
    with_clocks(&io, false);
    enable(&io, &store);
    assert!(
        store
            .load()
            .unwrap()
            .unwrap()
            .clock_approval
            .driver_identity
            .is_none()
    );
    transaction::execute(&io, &store, Trigger::SupplyChange, &mut || Ok(())).unwrap();
    assert_eq!(
        store
            .load()
            .unwrap()
            .unwrap()
            .clock_approval
            .driver_identity
            .as_deref(),
        Some("d".repeat(64).as_str())
    );
    io.0.borrow_mut().driver_identity = "e".repeat(64);
    let before = io.0.borrow().clock_writes;
    assert!(transaction::execute(&io, &store, Trigger::SupplyChange, &mut || Ok(())).is_err());
    assert_eq!(io.0.borrow().clock_writes, before);
}

#[test]
#[cfg(not(control_sim))]
fn controller_wake_preview_and_execution_use_the_event_scope_even_if_full_review_is_unavailable() {
    use crate::hal::power_events::{Event, Source};
    let (_dir, store, io) = setup();
    with_clocks(&io, true);
    let controller = Controller::new(store.0.clone(), store.0.with_file_name("profiles.json"));
    controller.shared.lock().unwrap().scheduler.ready().unwrap();
    controller
        .shared
        .observe(Event::Source(Source::Ac), 0)
        .unwrap();
    let view = controller.read_in_control(&io).unwrap();
    controller
        .enable_in_control(&io, &view.view.revision, 0, &mut || Ok(()))
        .unwrap();
    let work = controller
        .shared
        .lock()
        .unwrap()
        .scheduler
        .take(1_000)
        .unwrap();
    let (revision, _) = controller.preview_work(&io, work).unwrap();
    controller
        .execute_in_control(&io, work, &revision, &mut || Ok(()))
        .unwrap();
    io.0.borrow_mut().driver_error = Some("隔离驱动未就绪".into());
    let status = controller.read_in_control(&io).unwrap();
    assert!(status.view.enabled && status.active && status.view.error.is_some());
    controller.shared.observe(Event::Suspend, 2_000).unwrap();
    controller.shared.observe(Event::Resume, 3_000).unwrap();
    controller
        .shared
        .observe(Event::Source(Source::Ac), 3_000)
        .unwrap();
    assert!(
        controller
            .next_work(Source::Ac, 3_000, false)
            .unwrap()
            .is_none()
    );
    let work = controller
        .next_work(Source::Ac, 4_000, false)
        .unwrap()
        .unwrap();
    assert_eq!(work.trigger, Trigger::Resume);
    let (revision, _) = controller.preview_work(&io, work).unwrap();
    let applied = controller
        .execute_in_control(&io, work, &revision, &mut || Ok(()))
        .unwrap()
        .unwrap();
    assert_eq!(applied.trigger, Trigger::Resume);
}
