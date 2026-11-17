use super::*;
use crate::platform::lan::{
    self,
    model::{Field, Value},
};
use std::sync::atomic::Ordering;

fn target(fixture: &lan::test_fixture::Fixture) -> lan::Target {
    let mut target = lan::Target::capture(&fixture.io.frame.lock().unwrap())
        .unwrap()
        .unwrap();
    target.devices[0]
        .values
        .insert(Field::PowerSavingMode, Some(Value::string("1")));
    target
}

#[test]
fn software_process_lan_without_authorization_never_releases_the_running_curve() {
    let f = Fixture::new();
    let hal = f.hal();
    let lan = lan::test_fixture::Fixture::new();
    lan.enable();
    let target = target(&lan);
    let mut scope = lan.controller.scope().unwrap();
    let wanted = curves();
    {
        let _gate = hal.begin_write().unwrap();
        fanctl::software::apply(hal, &wanted).unwrap();
        let plan = scope.prepare_profile(hal, &target).unwrap().unwrap();
        let before = DEVICE.lock().unwrap().writes.len();
        let mut touched = false;
        assert!(scope.execute(hal, &plan, &mut touched, true).is_err());
        assert!(!touched);
        assert_eq!(DEVICE.lock().unwrap().writes.len(), before);
        assert_eq!(fanctl::software::read(hal).unwrap(), Some(wanted));
        assert_eq!(lan.io.authorizations.load(Ordering::SeqCst), 0);
        fanctl::set_mode(hal, 0).unwrap();
    }
}

#[test]
fn software_process_lan_authorization_keeps_running_curve_then_pauses_and_resumes_for_apply_and_rollback()
 {
    let f = Fixture::new();
    let hal = f.hal();
    let lan = lan::test_fixture::Fixture::new();
    lan.enable();
    let target = target(&lan);
    let mut scope = lan.controller.scope().unwrap();
    let wanted = curves();
    let plan = {
        let _gate = hal.begin_write().unwrap();
        fanctl::software::apply(hal, &wanted).unwrap();
        scope.prepare_profile(hal, &target).unwrap().unwrap()
    };
    let count = DEVICE.lock().unwrap().writes.len();
    scope.authorize_profile(Some(&plan)).unwrap();
    assert_eq!(fanctl::software::read(hal).unwrap(), Some(wanted.clone()));
    assert_eq!(DEVICE.lock().unwrap().writes.len(), count);
    {
        let _gate = hal.begin_write().unwrap();
        let mut touched = false;
        scope.execute(hal, &plan, &mut touched, true).unwrap();
        assert!(touched);
        assert_eq!(fanctl::software::read(hal).unwrap(), Some(wanted.clone()));
        assert_eq!(
            lan.io.frame.lock().unwrap().devices[0].power_saving(),
            Some(true)
        );
        scope.rollback(hal, &plan, &mut touched, true).unwrap();
        assert!(!touched);
        assert_eq!(fanctl::software::read(hal).unwrap(), Some(wanted.clone()));
        assert_eq!(
            lan.io.frame.lock().unwrap().devices[0].power_saving(),
            Some(false)
        );
    }
    scope.finish().unwrap();
    fanctl::software::tick(hal).unwrap();
    assert_eq!(lan.io.authorizations.load(Ordering::SeqCst), 1);
    let _gate = hal.begin_write().unwrap();
    fanctl::set_mode(hal, 0).unwrap();
}
#[test]
fn software_process_lan_resumes_the_curve_selected_by_an_earlier_profile_step() {
    let f = Fixture::new();
    let hal = f.hal();
    let lan = lan::test_fixture::Fixture::new();
    lan.enable();
    let target = target(&lan);
    let mut scope = lan.controller.scope().unwrap();
    let plan = {
        let _gate = hal.begin_write().unwrap();
        fanctl::software::apply(hal, &curves()).unwrap();
        scope.prepare_profile(hal, &target).unwrap().unwrap()
    };
    scope.authorize_profile(Some(&plan)).unwrap();
    let mut changed = curves();
    changed.get_mut("cpu").unwrap().points[0].duty_pct = 45.0;
    {
        let _gate = hal.begin_write().unwrap();
        fanctl::software::apply(hal, &changed).unwrap();
        let mut touched = false;
        scope.execute(hal, &plan, &mut touched, true).unwrap();
        assert_eq!(fanctl::software::read(hal).unwrap(), Some(changed.clone()));
        scope.rollback(hal, &plan, &mut touched, true).unwrap();
        assert_eq!(fanctl::software::read(hal).unwrap(), Some(changed));
    }
    scope.finish().unwrap();
    let _gate = hal.begin_write().unwrap();
    fanctl::set_mode(hal, 0).unwrap();
}
#[test]
fn software_process_lan_stop_failure_recovers_the_curve_before_any_network_write() {
    let f = Fixture::new();
    let hal = f.hal();
    let lan = lan::test_fixture::Fixture::new();
    lan.enable();
    let target = target(&lan);
    let mut scope = lan.controller.scope().unwrap();
    let wanted = curves();
    let plan = {
        let _gate = hal.begin_write().unwrap();
        fanctl::software::apply(hal, &wanted).unwrap();
        scope.prepare_profile(hal, &target).unwrap().unwrap()
    };
    scope.authorize_profile(Some(&plan)).unwrap();
    DEVICE.lock().unwrap().fail_release = true;
    {
        let _gate = hal.begin_write().unwrap();
        let mut touched = false;
        assert!(scope.execute(hal, &plan, &mut touched, true).is_err());
        assert!(!touched);
        assert_eq!(fanctl::software::read(hal).unwrap(), Some(wanted));
        assert_eq!(lan.io.executions.load(Ordering::SeqCst), 0);
        assert!(!lan.path().exists());
    }
    scope.finish().unwrap();
    fanctl::software::tick(hal).unwrap();
    let _gate = hal.begin_write().unwrap();
    fanctl::set_mode(hal, 0).unwrap();
}
