use super::*;
use crate::platform::lan::Controller;
use std::sync::{Arc, atomic::Ordering};
#[path = "../../profiles/apply/fixtures.rs"]
mod hal_fixture;

struct Fixture {
    hal: hal_fixture::Fixture,
    lan: super::super::test_fixture::Fixture,
}
impl Fixture {
    fn new() -> Self {
        let hal = hal_fixture::Fixture::new();
        let lan = super::super::test_fixture::Fixture::new();
        lan.enable();
        Self { hal, lan }
    }
    fn request(&self) -> Request {
        let profile = self.lan.controller.read(&self.hal.hal).unwrap().unwrap();
        Request {
            expected_revision: profile.revision,
            ..request(&device())
        }
    }
}

#[test]
fn missing_environment_never_falls_back_to_native_registry_or_authorization() {
    let f = Fixture::new();
    let controller = Controller::new(f.lan.controller.path.clone());
    assert!(
        controller
            .read(&f.hal.hal)
            .err()
            .unwrap()
            .to_string()
            .contains("未注入环境")
    );
    assert!(!f.lan.controller.path.exists());
}
#[test]
fn apply_authorizes_outside_hal_then_executes_with_the_same_persistent_first_baseline() {
    let f = Fixture::new();
    let request = f.request();
    assert_eq!(
        f.lan
            .controller
            .preview(&f.hal.hal, &request)
            .unwrap()
            .len(),
        3
    );
    assert!(!f.lan.controller.path.exists());
    f.lan.controller.apply(&f.hal.hal, &request).unwrap();
    let profile = f.lan.controller.read(&f.hal.hal).unwrap().unwrap();
    assert_eq!(profile.frame.devices[0].power_saving(), Some(true));
    assert_eq!(profile.initial[0].fields.len(), 3);
    f.lan
        .controller
        .restore(&f.hal.hal, &profile.revision, &request.device_id)
        .unwrap();
    assert_eq!(f.lan.io.frame.lock().unwrap().devices[0], device());
    assert_eq!(
        store::read(&f.lan.controller.path).unwrap().devices[0].fields[&Field::PowerSavingMode],
        Some(Value::string("0"))
    );
    assert_eq!(f.lan.io.executions.load(Ordering::SeqCst), 2);
}
#[test]
fn authorization_changes_to_source_or_record_cancel_before_execution() {
    for change in 0..4 {
        let f = Fixture::new();
        let request = f.request();
        let shared = f.lan.io.clone();
        let path = f.lan.controller.path.clone();
        *f.lan.io.on_authorize.lock().unwrap() = Some(Box::new(move || {
            if change == 3 {
                let mut record = Record::default();
                record.last_error = Some("隔离外部记录".into());
                store::write(&path, &Record::default(), &record).unwrap();
            } else {
                let mut frame = shared.frame.lock().unwrap();
                match change {
                    0 => frame.devices[0].identity.driver_version = Some(Value::string("2.0")),
                    1 => {
                        frame.devices[0]
                            .values
                            .insert(Field::WakeOnLink, Some(Value::string("external")));
                    }
                    _ => frame.capabilities.power_saving = false,
                }
            }
        }));
        assert!(
            f.lan
                .controller
                .apply(&f.hal.hal, &request)
                .unwrap_err()
                .to_string()
                .contains("授权期间")
        );
        assert_eq!(f.lan.io.executions.load(Ordering::SeqCst), 0);
        assert_eq!(f.lan.io.cancellations.load(Ordering::SeqCst), 1);
        assert_eq!(f.lan.controller.path.exists(), change == 3);
    }
}
#[test]
fn newly_running_owner_and_denied_authorization_leave_no_initial_record() {
    for owner in [false, true] {
        let f = Fixture::new();
        let request = f.request();
        if owner {
            let shared = f.lan.io.clone();
            *f.lan.io.on_authorize.lock().unwrap() = Some(Box::new(move || {
                shared.owner.store(true, Ordering::SeqCst);
            }));
        } else {
            f.lan.io.deny.store(true, Ordering::SeqCst);
        }
        assert!(f.lan.controller.apply(&f.hal.hal, &request).is_err());
        assert_eq!(f.lan.io.executions.load(Ordering::SeqCst), 0);
        assert!(!f.lan.controller.path.exists());
    }
}
#[test]
fn worker_checks_the_source_again_after_parent_permits_execution() {
    let f = Fixture::new();
    let request = f.request();
    let shared = f.lan.io.clone();
    *f.lan.io.before_execute.lock().unwrap() = Some(Box::new(move || {
        shared.frame.lock().unwrap().devices[0]
            .identity
            .driver_version = Some(Value::string("3.0"));
    }));
    assert!(f.lan.controller.apply(&f.hal.hal, &request).is_err());
    assert!(!f.lan.controller.path.exists());
    assert_eq!(
        f.lan.io.frame.lock().unwrap().devices[0].power_saving(),
        Some(false)
    );
}
#[test]
fn noop_and_stale_preview_never_request_authorization() {
    let f = Fixture::new();
    let mut request = f.request();
    request.power_saving = Some(false);
    request.apply_wake_patch = false;
    f.lan.controller.apply(&f.hal.hal, &request).unwrap();
    request.expected_revision = "stale".into();
    assert!(f.lan.controller.apply(&f.hal.hal, &request).is_err());
    assert_eq!(f.lan.io.authorizations.load(Ordering::SeqCst), 0);
    assert!(!f.lan.controller.path.exists());
}
#[test]
fn other_hal_work_can_run_during_authorization_without_allowing_a_stale_execution() {
    let f = Fixture::new();
    let request = f.request();
    let barrier = Arc::new(std::sync::Barrier::new(2));
    let wait = barrier.clone();
    *f.lan.io.on_authorize.lock().unwrap() = Some(Box::new(move || {
        wait.wait();
        wait.wait();
    }));
    std::thread::scope(|scope| {
        let operation = scope.spawn(|| f.lan.controller.apply(&f.hal.hal, &request));
        barrier.wait();
        {
            let _gate = f.hal.hal.begin_write().unwrap();
            f.lan.io.owner.store(true, Ordering::SeqCst);
        }
        barrier.wait();
        assert!(operation.join().unwrap().is_err());
    });
    assert_eq!(f.lan.io.executions.load(Ordering::SeqCst), 0);
}
