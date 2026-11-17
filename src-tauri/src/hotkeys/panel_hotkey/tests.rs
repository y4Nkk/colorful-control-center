mod fixtures;
mod transport;
use super::*;
use ccc_core::panel_hotkey::Activity;
use fixtures::*;

#[test]
fn confirmed_oled_precedes_flags_and_partial_unknown_nodes_but_non_oled_requires_complete_inventory()
 {
    let mut source = source(true);
    source
        .inventory
        .unresolved
        .push("另一个节点读取失败".into());
    assert_eq!(
        Source::new(source.inventory, None, source.saver)
            .unwrap()
            .kind,
        Kind::BlankSaver
    );
    let mut inventory = inventory(false);
    inventory.unresolved.push("节点未知".into());
    assert!(Source::new(inventory, Some(&[0, 0]), None).is_err());
    assert!(Source::new(fixtures::inventory(false), Some(&[2, 0]), None).is_err());
    let mut flags = [0; 16];
    flags[0] = 1;
    flags[15] = 8;
    assert!(Source::new(fixtures::inventory(false), Some(&flags), None).is_err());
}
#[test]
fn preview_is_readonly_and_cleanup_remains_visible_when_source_becomes_unavailable() {
    let controller = Controller::default();
    let io = Memory::new(false);
    assert!(controller.preview(&io).unwrap().session.is_none());
    assert_eq!(io.0.lock().unwrap().prepares, 0);
    io.begin(&controller).unwrap();
    io.0.lock().unwrap().broken = true;
    let profile = controller.preview(&io).unwrap();
    assert!(profile.identity.is_none() && profile.error.is_some());
    controller.end(profile.restore_revision.as_deref()).unwrap();
    assert!(!controller.active().unwrap());
}
#[test]
fn source_change_after_prepare_and_revoked_permit_never_activate_and_clean_prepared_resource() {
    for changed in [true, false] {
        let controller = Controller::default();
        let io = Memory::new(true);
        if changed {
            io.0.lock().unwrap().after_prepare = Some(source(false));
        }
        let mut calls = 0;
        let result = controller.begin(
            &io,
            &io.id(),
            || {
                calls += 1;
                Ok(changed || calls == 1)
            },
            || Ok(true),
        );
        if changed {
            assert!(result.is_err());
        } else {
            assert!(result.unwrap().is_none());
        }
        let data = io.0.lock().unwrap();
        assert_eq!((data.starts, data.stops, data.wakes), (0, 1, 0));
        assert!(!controller.active().unwrap());
    }
}
#[test]
fn settling_cancellation_has_no_start_and_initial_cancellation_does_not_prepare() {
    for first in [true, false] {
        let controller = Controller::default();
        let io = Memory::new(true);
        assert!(
            controller
                .begin(&io, &io.id(), || Ok(first), || Ok(false))
                .unwrap()
                .is_none()
        );
        let data = io.0.lock().unwrap();
        assert_eq!(
            (data.prepares, data.starts, data.stops),
            if first { (1, 0, 1) } else { (0, 0, 0) }
        );
    }
}
#[test]
fn repeated_event_is_idempotent_and_old_preview_cannot_end_a_new_session() {
    let controller = Controller::default();
    let io = Memory::new(true);
    io.begin(&controller).unwrap();
    let old = controller.restore_revision().unwrap().unwrap();
    io.begin(&controller).unwrap();
    assert_eq!(io.0.lock().unwrap().prepares, 1);
    controller.end(Some(&old)).unwrap();
    io.begin(&controller).unwrap();
    assert!(controller.end(Some(&old)).is_err());
    assert!(controller.active().unwrap());
    controller.end(None).unwrap();
    assert!(!controller.end(None).unwrap());
    assert_eq!(io.0.lock().unwrap().stops, 2);
}
#[test]
fn frozen_confirmed_session_blocks_new_activity_until_listener_has_stopped() {
    let controller = Controller::default();
    let io = Memory::new(false);
    io.begin(&controller).unwrap();
    let revision = controller.restore_revision().unwrap().unwrap();
    let freeze = controller.freeze(&revision).unwrap();
    io.0.lock()
        .unwrap()
        .events
        .push_back(Event::Input(Activity::Pointer));
    assert!(
        controller
            .poll(&io, || panic!("冻结期间不能请求许可"))
            .unwrap()
            .is_none()
    );
    assert!(io.begin(&controller).unwrap().is_none());
    assert_eq!(io.0.lock().unwrap().wakes, 0);
    controller.end(None).unwrap();
    drop(freeze);
    io.0.lock().unwrap().events.clear();
    io.begin(&controller).unwrap();
    assert!(controller.freeze(&revision).is_err());
    assert!(controller.active().unwrap());
    controller.end(None).unwrap();
}
#[test]
fn confirmation_of_preparing_session_is_rejected_if_that_session_finishes_starting() {
    let controller = std::sync::Arc::new(Controller::default());
    let io = Memory::new(true);
    let (ready, waiting) = std::sync::mpsc::sync_channel(1);
    let (resume, paused) = std::sync::mpsc::sync_channel(1);
    let worker = {
        let controller = controller.clone();
        let io = io.clone();
        std::thread::spawn(move || {
            controller.begin(
                &io,
                &io.id(),
                || Ok(true),
                || {
                    ready.send(()).unwrap();
                    paused
                        .recv_timeout(std::time::Duration::from_secs(5))
                        .unwrap();
                    Ok(true)
                },
            )
        })
    };
    waiting
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    let revision = controller.restore_revision().unwrap().unwrap();
    let confirmation = {
        let controller = controller.clone();
        std::thread::spawn(move || controller.freeze(&revision).is_ok())
    };
    resume.send(()).unwrap();
    assert!(worker.join().unwrap().unwrap().is_some());
    assert!(!confirmation.join().unwrap());
    assert!(controller.active().unwrap());
    controller.end(None).unwrap();
}
#[test]
fn preparation_start_and_poll_failures_leave_no_active_resource_after_cleanup() {
    for stage in 0..3 {
        let controller = Controller::default();
        let io = Memory::new(true);
        {
            let mut data = io.0.lock().unwrap();
            data.prepare_fail = stage == 0;
            data.start_fail = stage == 1;
            data.poll_fail = stage == 2;
        }
        let started = io.begin(&controller);
        if stage < 2 {
            assert!(started.is_err());
        } else {
            started.unwrap();
            assert!(controller.poll(&io, || Ok(true)).is_err());
        }
        assert!(!controller.active().unwrap());
        assert_eq!(io.0.lock().unwrap().wakes, 0);
    }
}
#[test]
fn cleanup_failure_retains_identity_and_resource_for_explicit_retry() {
    let controller = Controller::default();
    let io = Memory::new(true);
    io.begin(&controller).unwrap();
    io.0.lock().unwrap().stop_fail = true;
    assert!(controller.end(None).is_err());
    let profile = controller.preview(&io).unwrap();
    assert!(profile.session.unwrap().error.is_some());
    assert!(io.begin(&controller).is_err());
    io.0.lock().unwrap().stop_fail = false;
    controller.end(profile.restore_revision.as_deref()).unwrap();
    assert_eq!(io.0.lock().unwrap().stops, 2);
}
#[test]
fn poll_and_cleanup_errors_are_both_preserved_and_cleanup_can_be_retried() {
    let controller = Controller::default();
    let io = Memory::new(true);
    io.begin(&controller).unwrap();
    {
        let mut data = io.0.lock().unwrap();
        data.poll_fail = true;
        data.stop_fail = true;
    }
    let error = format!("{:#}", controller.poll(&io, || Ok(true)).unwrap_err());
    assert!(error.contains("资源状态读取失败"));
    assert!(error.contains("结束资源失败"));
    assert!(controller.active().unwrap());
    io.0.lock().unwrap().stop_fail = false;
    controller.end(None).unwrap();
    assert!(!controller.active().unwrap());
}
#[test]
fn blank_exit_is_reaped_and_nonzero_status_is_reported_without_wake() {
    for code in [0, 7, 259] {
        let controller = Controller::default();
        let io = Memory::new(true);
        io.begin(&controller).unwrap();
        io.0.lock().unwrap().events.push_back(Event::Exited(code));
        let result = controller.poll(&io, || panic!("屏保结束不需要新写入许可"));
        assert_eq!(result.is_ok(), code == 0);
        assert!(!controller.active().unwrap());
        assert_eq!(io.0.lock().unwrap().wakes, 0);
    }
}
#[test]
fn keyboard_ends_wait_without_requery_or_wake_and_pointer_wakes_exactly_once_after_cleanup() {
    for activity in [Activity::KeyDown, Activity::Pointer] {
        let controller = Controller::default();
        let io = Memory::new(false);
        io.begin(&controller).unwrap();
        let before = io.0.lock().unwrap().reads;
        io.0.lock()
            .unwrap()
            .events
            .push_back(Event::Input(activity));
        controller.poll(&io, || Ok(true)).unwrap();
        controller.poll(&io, || Ok(true)).unwrap();
        let data = io.0.lock().unwrap();
        assert_eq!(data.stops, 1);
        assert_eq!(data.wakes, usize::from(activity == Activity::Pointer));
        if activity == Activity::KeyDown {
            assert_eq!(data.reads, before);
        }
    }
}
#[test]
fn pointer_wake_is_cancelled_on_stale_source_revocation_or_cleanup_failure_and_failed_wake_is_not_replayed()
 {
    for stage in 0..4 {
        let controller = Controller::default();
        let io = Memory::new(false);
        io.begin(&controller).unwrap();
        {
            let mut data = io.0.lock().unwrap();
            data.events.push_back(Event::Input(Activity::Pointer));
            if stage == 0 {
                data.source.inventory.monitors[0].fingerprint = "b".repeat(64);
            }
            data.stop_fail = stage == 2;
            data.wake_fail = stage == 3;
        }
        let result = controller.poll(&io, || Ok(stage != 1));
        assert_eq!(result.is_ok(), stage == 1);
        assert_eq!(io.0.lock().unwrap().wakes, usize::from(stage == 3));
        if stage == 2 {
            io.0.lock().unwrap().stop_fail = false;
            controller.end(None).unwrap();
        }
        assert!(controller.poll(&io, || Ok(true)).unwrap().is_none());
        assert!(!controller.active().unwrap());
    }
}
#[test]
fn pending_input_never_queries_source_or_requests_wake_and_first_activity_is_one_shot() {
    let controller = Controller::default();
    let io = Memory::new(false);
    io.begin(&controller).unwrap();
    let before = io.0.lock().unwrap().reads;
    controller.poll(&io, || panic!("尚无活动")).unwrap();
    assert_eq!(io.0.lock().unwrap().reads, before);
    let (input, receiver) = resource::Input::channel();
    input.submit(Activity::Pointer);
    assert!(receiver.try_recv().is_err());
    input.activate();
    input.submit(Activity::KeyDown);
    input.submit(Activity::Pointer);
    assert_eq!(receiver.try_recv().unwrap(), Activity::KeyDown);
    assert!(receiver.try_recv().is_err());
    input.stop();
    input.submit(Activity::Pointer);
    assert!(receiver.try_recv().is_err());
    controller.end(None).unwrap();
}
