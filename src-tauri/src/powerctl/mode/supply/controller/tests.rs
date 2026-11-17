use super::*;
use crate::{
    hal::power_events::{Event, Source},
    powerctl::mode::supply::{
        control::Shared,
        transaction_tests::{Memory, setup},
    },
};
use ccc_core::oem_supply_mode::Trigger;

fn controller(store: &FileStore) -> Controller {
    let controller = Controller::new(store.0.clone(), store.0.with_file_name("profiles.json"));
    controller.shared.lock().unwrap().scheduler.ready().unwrap();
    controller
        .shared
        .observe(Event::Source(Source::Battery), 0)
        .unwrap();
    controller
}
fn enabled(controller: &Controller, io: &Memory) -> Work {
    let view = controller.read_in_control(io).unwrap();
    controller
        .enable_in_control(io, &view.view.revision, 0, &mut || Ok(()))
        .unwrap();
    controller
        .shared
        .lock()
        .unwrap()
        .scheduler
        .take(1_000)
        .unwrap()
}
#[test]
fn explicit_pause_consumes_delayed_startup_without_deleting_the_saved_policy() {
    let (_dir, store, io) = setup();
    let original = controller(&store);
    enabled(&original, &io);
    let reopened = controller(&store);
    reopened.pause().unwrap();
    assert!(reopened.shared.claim_startup().unwrap().is_none());
    assert!(store.load().unwrap().unwrap().enabled);
    assert_eq!(io.0.borrow().writes, 0);
}
#[test]
fn a_pause_after_startup_is_claimed_prevents_late_configuration() {
    let (_dir, store, _io) = setup();
    let controller = controller(&store);
    let epoch = controller.shared.claim_startup().unwrap().unwrap();
    controller.shared.permit_setup(epoch).unwrap();
    controller.pause().unwrap();
    assert!(
        controller
            .shared
            .configure(epoch, Source::Battery, 0)
            .is_err()
    );
    assert!(!controller.shared.lock().unwrap().scheduler.enabled);
    assert!(controller.shared.claim_startup().unwrap().is_none());
}
#[test]
fn explicit_enable_keeps_its_queued_work_instead_of_restarting_saved_policy() {
    let (_dir, store, io) = setup();
    let controller = controller(&store);
    let work = enabled(&controller, &io);
    assert!(controller.shared.claim_startup().unwrap().is_none());
    assert!(controller.shared.permit(work.epoch, work.source).is_ok());
    assert_eq!(io.0.borrow().writes, 0);
}
#[test]
fn confirmation_enables_one_startup_work_without_direct_device_outputs() {
    let (_dir, store, io) = setup();
    let controller = controller(&store);
    let work = enabled(&controller, &io);
    assert_eq!(work.trigger, Trigger::Startup);
    assert_eq!(io.0.borrow().writes, 0);
    let view = controller.read_in_control(&io).unwrap();
    assert!(view.active && !view.can_resume && !view.view.can_enable);
    let (revision, _) = controller.preview_work(&io, work).unwrap();
    let applied = controller
        .execute_in_control(&io, work, &revision, &mut || Ok(()))
        .unwrap()
        .unwrap();
    assert_eq!((applied.previous_mode, applied.mode), (2, 1));
    assert_eq!(io.0.borrow().writes, 1);
    assert!(
        controller
            .shared
            .lock()
            .unwrap()
            .scheduler
            .take(2_000)
            .is_none()
    );
}

#[test]
fn ipc_status_is_flat_and_exposes_six_conditions_without_internal_snapshots() {
    let (_dir, store, io) = setup();
    let controller = controller(&store);
    let status = serde_json::to_value(controller.read_in_control(&io).unwrap()).unwrap();
    assert!(status.get("view").is_none() && status.get("current").is_none());
    assert_eq!(status["listenerOnline"], true);
    assert_eq!(status["canEnable"], true);
    assert_eq!(status["review"]["startupAc"]["mode"], 2);
    assert_eq!(status["review"]["resumeBattery"]["mode"], 1);
    for group in ["gpu", "clocks"] {
        assert!(status["review"][group].get("startupAc").is_some());
        assert!(status["review"][group].get("resumeBattery").is_some());
        assert!(status["review"][group].get("startup").is_none());
        assert!(status["review"][group].get("resume").is_none());
    }
    assert!(status["review"].get("energy").is_none());
    assert_eq!(status["review"]["currentEnergy"]["energyStar"], 0);
    for key in [
        "ac",
        "battery",
        "startupAc",
        "startupBattery",
        "resumeAc",
        "resumeBattery",
    ] {
        assert!(status["review"][key]["energy"].is_object());
    }
    assert_eq!(io.0.borrow().writes, 0);
}
#[test]
fn pause_revokes_frozen_work_and_explicit_resume_gets_a_new_epoch() {
    let (_dir, store, io) = setup();
    let controller = controller(&store);
    let old = enabled(&controller, &io);
    let (revision, _) = controller.preview_work(&io, old).unwrap();
    controller.pause().unwrap();
    assert!(
        controller
            .execute_in_control(&io, old, &revision, &mut || Ok(()))
            .is_err()
    );
    assert!(store.load().unwrap().unwrap().enabled);
    assert!(controller.read_in_control(&io).unwrap().can_resume);
    controller
        .resume_in_control(&io, 2_000, &mut || Ok(()))
        .unwrap();
    let new = controller
        .shared
        .lock()
        .unwrap()
        .scheduler
        .take(3_000)
        .unwrap();
    assert_ne!(new.epoch, old.epoch);
    assert_eq!(io.0.borrow().writes, 0);
}
#[test]
fn source_notification_revokes_authorization_before_actual_state_reconciles() {
    let (_dir, store, io) = setup();
    let controller = controller(&store);
    let work = enabled(&controller, &io);
    let (revision, _) = controller.preview_work(&io, work).unwrap();
    controller
        .shared
        .observe(Event::Source(Source::Ac), 20)
        .unwrap();
    assert!(
        controller
            .execute_in_control(&io, work, &revision, &mut || Ok(()))
            .is_err()
    );
    io.0.borrow_mut().review.on_ac = Some(true);
    controller
        .shared
        .lock()
        .unwrap()
        .scheduler
        .read_source(Source::Ac, 30)
        .unwrap();
    let next = controller
        .shared
        .lock()
        .unwrap()
        .scheduler
        .take(1_030)
        .unwrap();
    assert_eq!(next.trigger, Trigger::SupplyChange);
    assert_eq!(next.source, Source::Ac);
    assert_eq!(io.0.borrow().writes, 0);
}
#[test]
fn changes_during_authorization_reject_the_complete_old_read_set() {
    let (_dir, store, io) = setup();
    let controller = controller(&store);
    let work = enabled(&controller, &io);
    let (revision, _) = controller.preview_work(&io, work).unwrap();
    io.0.borrow_mut().review.current_mode = 3;
    assert!(
        controller
            .execute_in_control(&io, work, &revision, &mut || Ok(()))
            .is_err()
    );
    assert!(store.load().unwrap().unwrap().initial.is_none());
    assert_eq!(io.0.borrow().writes, 0);
}
#[test]
fn changed_or_corrupted_journal_is_not_accepted_as_new_authorization() {
    let (_dir, store, io) = setup();
    let controller = controller(&store);
    let work = enabled(&controller, &io);
    let (revision, _) = controller.preview_work(&io, work).unwrap();
    let mut record = store.load().unwrap().unwrap();
    let expected = record.clone();
    record.identity = "f".repeat(64);
    store.save(Some(&expected), &record).unwrap();
    assert!(
        controller
            .execute_in_control(&io, work, &revision, &mut || Ok(()))
            .is_err()
    );
    std::fs::write(&store.0, b"invalid journal").unwrap();
    assert!(
        controller
            .resume_in_control(&io, 5_000, &mut || Ok(()))
            .is_err()
    );
    assert_eq!(std::fs::read(&store.0).unwrap(), b"invalid journal");
    assert_eq!(io.0.borrow().writes, 0);
}
#[test]
fn another_automatic_owner_blocks_enable_and_revokes_already_frozen_work() {
    let (_dir, store, io) = setup();
    let controller = controller(&store);
    let work = enabled(&controller, &io);
    let (revision, _) = controller.preview_work(&io, work).unwrap();
    let policy = serde_json::json!({"version":2,"enabled":true,"ac":{
        "profile":"隔离档案","digest":"c".repeat(64),"mode":null},"battery":null,"resume":null});
    std::fs::write(
        &controller.profiles_path,
        serde_json::to_vec(&policy).unwrap(),
    )
    .unwrap();
    assert!(
        crate::profiles::policy::ensure_inactive(&controller.profiles_path)
            .unwrap_err()
            .to_string()
            .contains("仍已启用")
    );
    assert!(!controller.read_in_control(&io).unwrap().active);
    assert!(
        controller
            .execute_in_control(&io, work, &revision, &mut || Ok(()))
            .is_err()
    );
    store.clear(&store.load().unwrap().unwrap()).unwrap();
    let view = controller.read_in_control(&io).unwrap();
    assert!(!view.view.can_enable);
    assert!(
        controller
            .enable_in_control(&io, &view.view.revision, 0, &mut || Ok(()))
            .is_err()
    );
    assert_eq!(io.0.borrow().writes, 0);
}
#[test]
fn a_reopened_controller_only_resumes_an_unchanged_explicit_saved_policy() {
    let (_dir, store, io) = setup();
    let original = controller(&store);
    enabled(&original, &io);
    let reopened = controller(&store);
    reopened
        .resume_in_control(&io, 5_000, &mut || Ok(()))
        .unwrap();
    assert_eq!(
        reopened
            .shared
            .lock()
            .unwrap()
            .scheduler
            .take(6_000)
            .unwrap()
            .trigger,
        Trigger::Startup
    );
    let original_record = store.load().unwrap().unwrap();
    io.0.borrow_mut().review.identity = "d".repeat(64);
    let changed = controller(&store);
    assert!(
        changed
            .resume_in_control(&io, 7_000, &mut || Ok(()))
            .is_err()
    );
    assert!(store.load().unwrap().unwrap() == original_record);
    assert_eq!(io.0.borrow().writes, 0);
}
#[test]
fn offline_or_sleeping_notifications_cannot_enable_and_failures_revoke_existing_work() {
    let (_dir, store, io) = setup();
    let controller = Controller::new(store.0.clone(), store.0.with_file_name("profiles.json"));
    let view = controller.read_in_control(&io).unwrap();
    assert!(!view.view.can_enable);
    assert!(
        controller
            .enable_in_control(&io, &view.view.revision, 0, &mut || Ok(()))
            .is_err()
    );
    controller.shared.lock().unwrap().scheduler.ready().unwrap();
    controller.shared.observe(Event::Suspend, 0).unwrap();
    assert!(!controller.read_in_control(&io).unwrap().view.can_enable);
    controller.shared.observe(Event::Resume, 0).unwrap();
    let work = enabled(&controller, &io);
    controller.shared.failed("隔离通知失败".into());
    assert!(controller.shared.permit(work.epoch, work.source).is_err());
    assert_eq!(io.0.borrow().writes, 0);
}
#[test]
fn stop_and_sleep_revoke_restore_while_listener_failure_still_allows_manual_recovery() {
    let shared = Shared::default();
    let ticket = shared.lock().unwrap().scheduler.ticket();
    assert!(shared.permit_restore(ticket).is_ok());
    shared.failed("离线".into());
    assert!(shared.permit_restore(ticket).is_err());
    let ticket = shared.lock().unwrap().scheduler.ticket();
    assert!(shared.permit_restore(ticket).is_ok());
    shared.observe(Event::Suspend, 0).unwrap();
    assert!(shared.permit_restore(ticket).is_err());
    shared.lock().unwrap().scheduler.stop().unwrap();
    let ticket = shared.lock().unwrap().scheduler.ticket();
    assert!(shared.permit_restore(ticket).is_err());
}
#[test]
fn authorization_failure_keeps_the_initial_journal_empty_and_reports_no_confirmed_targets() {
    let (_dir, store, io) = setup();
    let controller = controller(&store);
    let work = enabled(&controller, &io);
    let (revision, _) = controller.preview_work(&io, work).unwrap();
    let error = controller
        .execute_in_control(&io, work, &revision, &mut || bail!("隔离许可撤销"))
        .unwrap_err();
    controller
        .shared
        .report(work.trigger, None, Some(error.to_string()), 10)
        .unwrap();
    let status = controller.read_in_control(&io).unwrap();
    assert!(!status.active && !status.last.unwrap().applied);
    assert!(store.load().unwrap().unwrap().initial.is_none());
    assert_eq!(io.0.borrow().writes, 0);
}

#[test]
fn an_explicit_resume_rechecks_current_facts_and_clears_a_historical_issue() {
    let (_dir, store, io) = setup();
    let controller = controller(&store);
    enabled(&controller, &io);
    controller
        .shared
        .pause(Some("此前的隔离错误".into()))
        .unwrap();
    assert!(controller.read_in_control(&io).unwrap().can_resume);
    controller
        .resume_in_control(&io, 2_000, &mut || Ok(()))
        .unwrap();
    let status = controller.read_in_control(&io).unwrap();
    assert!(status.active && status.issue.is_none());
    assert_eq!(io.0.borrow().writes, 0);
}

#[test]
fn refresh_waits_for_queued_or_executing_modes_and_for_matching_supply_evidence() {
    let (_dir, store, io) = setup();
    let controller = controller(&store);
    let view = controller.read_in_control(&io).unwrap();
    controller
        .enable_in_control(&io, &view.view.revision, 0, &mut || Ok(()))
        .unwrap();
    assert!(controller.waiting_for_mode(Some(false)).unwrap());
    let work = {
        let mut state = controller.shared.lock().unwrap();
        let work = state.scheduler.take(1_000).unwrap();
        state.executing = true;
        work
    };
    assert!(controller.waiting_for_mode(Some(false)).unwrap());
    controller
        .shared
        .report(
            work.trigger,
            Some(Applied {
                previous_mode: 2,
                mode: 1,
                on_ac: false,
                trigger: work.trigger,
            }),
            None,
            1_100,
        )
        .unwrap();
    assert!(!controller.waiting_for_mode(Some(false)).unwrap());
    assert!(controller.waiting_for_mode(Some(true)).unwrap());
    controller.pause().unwrap();
    assert!(!controller.waiting_for_mode(Some(true)).unwrap());
}

#[test]
fn a_cleanup_error_preserves_the_confirmed_mode_result_while_stopping_future_outputs() {
    let (_dir, store, io) = setup();
    let controller = controller(&store);
    let work = enabled(&controller, &io);
    let (revision, _) = controller.preview_work(&io, work).unwrap();
    let applied = controller
        .execute_in_control(&io, work, &revision, &mut || Ok(()))
        .unwrap();
    controller
        .shared
        .report(
            work.trigger,
            applied,
            Some("隔离授权收尾失败".into()),
            1_100,
        )
        .unwrap();
    let status = controller.read_in_control(&io).unwrap();
    let report = status.last.unwrap();
    assert!(!status.active && report.applied && report.error.is_some());
    assert_eq!((report.previous_mode, report.mode), (Some(2), Some(1)));
    assert!(store.load().unwrap().unwrap().last.is_some());
}
