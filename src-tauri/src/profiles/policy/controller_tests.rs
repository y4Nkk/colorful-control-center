use super::{Record, Trigger, fixture::Harness};
use crate::hal::power_events::{Event, Source};

#[test]
fn complete_save_binds_file_mode_and_first_restore_preserves_initial_absence() {
    let h = Harness::new(false);
    h.enable(true);
    let view = h.policy.read().unwrap();
    assert!(view.active && !view.paused && view.initial.is_some());
    assert!(
        view.record
            .as_ref()
            .unwrap()
            .binding(Trigger::Resume)
            .is_some()
    );
    assert!(h.last().is_none());
    assert!(h.f.device.0.lock().unwrap().events.is_empty());
    h.policy.pause().unwrap();
    let view = h.policy.read().unwrap();
    assert!(view.paused && !view.active);
    assert!(view.record.as_ref().unwrap().enabled);
    h.enable(false);
    h.policy.restore(&h.f.hal, &h.record()).unwrap();
    let view = h.policy.read().unwrap();
    assert!(view.paused && !view.active && view.initial.is_none());
    assert_eq!(view.record.unwrap(), Record::default());
    assert!(!h.dir.path().join("power-profile-policy.json").exists());
}

#[test]
fn changed_file_and_unreviewed_linked_mode_refuse_before_policy_commit() {
    let h = Harness::new(false);
    let request = h.request(false);
    let mut changed = h.file("电池");
    changed.settings.pl1_w = Some(60.0);
    h.write(&changed);
    assert!(
        h.policy
            .save(&h.context(), &h.profiles(), &request)
            .is_err()
    );
    assert!(!h.dir.path().join("power-profile-policy.json").exists());
    let mut request = h.request(false);
    let mut settings = request.ac.as_ref().unwrap().file.settings.clone();
    settings.oem_power_mode = Some(0);
    request.ac.as_mut().unwrap().mode =
        crate::profiles::preview_authorization(&h.context(), &settings).unwrap();
    assert!(
        h.policy
            .save(&h.context(), &h.profiles(), &request)
            .is_err()
    );
    assert!(h.f.device.0.lock().unwrap().events.is_empty());
}

#[test]
fn stale_confirmation_and_offline_listener_cannot_enable_new_rules() {
    let h = Harness::new(false);
    let stale = h.request(false);
    h.enable(true);
    assert!(h.policy.save(&h.context(), &h.profiles(), &stale).is_err());
    h.policy.shared.failed("隔离通知已退出".into());
    assert!(
        h.policy
            .save(&h.context(), &h.profiles(), &h.request(false))
            .is_err()
    );
    let view = h.policy.read().unwrap();
    assert!(!view.active && view.listener_error.unwrap().contains("已退出"));
}

#[test]
fn external_record_changes_revoke_permission_and_reload_never_implicitly_accepts_enable() {
    let h = Harness::new(false);
    h.enable(true);
    h.observe(Event::Source(Source::Battery), 1);
    let work = h.policy.shared.take(2_000).unwrap().unwrap();
    let permission = h
        .policy
        .shared
        .permission(work.epoch, work.record.clone(), work.trigger);
    assert!(permission.check().is_ok());
    let changed = Record {
        resume: None,
        ..h.record()
    };
    std::fs::write(
        h.dir.path().join("power-profile-policy.json"),
        serde_json::to_vec(&changed).unwrap(),
    )
    .unwrap();
    assert!(permission.check().is_err());
    let view = h.policy.read().unwrap();
    assert!(view.paused && !view.active && view.configuration_error.is_some());
    h.policy.reload().unwrap();
    let view = h.policy.read().unwrap();
    assert!(view.paused && !view.active && view.configuration_error.is_none());
    assert_eq!(view.record.unwrap(), changed);
    h.enable(false);
    assert!(h.policy.read().unwrap().active);
    assert!(h.tick(u64::MAX).is_ok() && h.last().is_none());
}

#[test]
fn callbacks_and_listener_failure_do_not_wait_for_configuration_io() {
    let h = Harness::new(false);
    h.enable(false);
    let config = h.policy.shared.lock_config().unwrap();
    let shared = h.policy.shared.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    let thread = std::thread::spawn(move || {
        shared.observe(Event::Source(Source::Battery), 1).unwrap();
        shared.failed("隔离监听故障".into());
        tx.send(()).unwrap();
    });
    let completed = rx.recv_timeout(std::time::Duration::from_secs(1));
    drop(config);
    thread.join().unwrap();
    assert!(completed.is_ok());
    assert!(!h.policy.read().unwrap().active);
}

#[test]
fn shutdown_revokes_permissions_and_never_reenables_from_late_ready() {
    let h = Harness::new(false);
    h.enable(false);
    h.observe(Event::Source(Source::Battery), 1);
    let work = h.policy.shared.take(2_000).unwrap().unwrap();
    let permission = h
        .policy
        .shared
        .permission(work.epoch, work.record, work.trigger);
    h.policy.stop().unwrap();
    h.policy
        .shared
        .ready(crate::hal::power_events::Route {
            modern_standby: Some(false),
            display_events: false,
            warnings: vec![],
        })
        .unwrap();
    assert!(permission.check().is_err());
    assert!(!h.policy.read().unwrap().active);
    assert!(
        h.policy
            .save(&h.context(), &h.profiles(), &h.request(false))
            .is_err()
    );
}

#[test]
fn corrupt_startup_record_remains_visible_and_cannot_be_overwritten_by_default() {
    let h = Harness::new(false);
    let path = h.dir.path().join("power-profile-policy.json");
    std::fs::write(&path, b"{broken").unwrap();
    let broken = super::super::controller::Controller::new(path.clone());
    broken
        .shared
        .ready(crate::hal::power_events::Route {
            modern_standby: Some(false),
            display_events: false,
            warnings: vec![],
        })
        .unwrap();
    let view = broken.read().unwrap();
    assert!(view.record.is_none() && view.configuration_error.is_some() && !view.active);
    broken.pause().unwrap();
    assert!(
        broken
            .save(&h.context(), &h.profiles(), &h.request(false))
            .is_err()
    );
    assert_eq!(std::fs::read(&path).unwrap(), b"{broken");
    std::fs::write(&path, serde_json::to_vec(&Record::default()).unwrap()).unwrap();
    broken.reload().unwrap();
    let view = broken.read().unwrap();
    assert!(view.record.is_some() && view.paused && !view.active);
}

#[test]
fn failed_policy_commit_retains_previous_rules_but_revokes_old_in_flight_epoch() {
    let h = Harness::new(false);
    h.enable(true);
    h.observe(Event::Source(Source::Battery), 1);
    let work = h.policy.shared.take(2_000).unwrap().unwrap();
    let permission = h
        .policy
        .shared
        .permission(work.epoch, work.record.clone(), work.trigger);
    let request = h.request(false);
    let path = h.dir.path().join("power-profile-policy.json");
    let mut permissions = std::fs::metadata(&path).unwrap().permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(&path, permissions).unwrap();
    let result = h.policy.save(&h.context(), &h.profiles(), &request);
    let mut permissions = std::fs::metadata(&path).unwrap().permissions();
    permissions.set_readonly(false);
    std::fs::set_permissions(&path, permissions).unwrap();
    assert!(result.is_err());
    assert!(permission.check().is_err());
    let view = h.policy.read().unwrap();
    assert!(view.active && !view.paused && view.initial.is_some());
    assert_eq!(view.record.unwrap(), work.record);
}
