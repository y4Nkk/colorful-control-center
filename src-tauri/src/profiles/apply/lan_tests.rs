use super::*;
use crate::platform::lan::{self, model::*, store};
use std::sync::{Arc, atomic::Ordering};
#[path = "lan_source_tests.rs"]
mod source;

fn setup() -> Fixture {
    let f = Fixture::new();
    f.lan.enable();
    f
}
fn captured(f: &Fixture) -> lan::Target {
    lan::Target::capture(&f.lan.io.frame.lock().unwrap())
        .unwrap()
        .unwrap()
}
fn wanted(f: &Fixture) -> ProfileSettings {
    let mut target = captured(f);
    for device in &mut target.devices {
        device
            .values
            .insert(Field::PowerSavingMode, Some(Value::string("1")));
        device
            .values
            .insert(Field::PnpCapabilities, Some(Value::dword(256)));
        device
            .values
            .insert(Field::WakeOnLink, Some(Value::string("2")));
    }
    ProfileSettings {
        lan_policy: Some(target),
        ..Default::default()
    }
}
fn apply(f: &Fixture, settings: &ProfileSettings) -> Vec<ApplyStep> {
    apply_settings(&f.hal, &f.gpu, f.devices(), settings).unwrap()
}
fn fail_gpu(f: &Fixture, settings: &mut ProfileSettings) {
    f.device.0.lock().unwrap().fail_gpu = true;
    settings.gpu = Some(crate::gpuctl::GpuClockSettings {
        core_offset_mhz: Some(60),
        mem_offset_mhz: None,
    });
}
fn initial(f: &Fixture) -> store::Record {
    store::read(&f.lan.path()).unwrap()
}
fn unchanged(f: &Fixture) {
    assert_eq!(
        f.lan.io.frame.lock().unwrap().devices[0],
        lan::test_fixture::device()
    );
}

#[test]
fn capture_and_file_roundtrip_keep_driver_identity_raw_type_and_missing_values_without_authorization()
 {
    let f = setup();
    f.lan.io.frame.lock().unwrap().devices[0].values.insert(
        Field::PnpCapabilities,
        Some(Value {
            kind: 3,
            bytes: vec![1, 9],
        }),
    );
    let settings = crate::profiles::capture_current(&f.hal, &f.gpu, f.devices(), &crate::profiles::ProfileGroup::ALL).unwrap();
    let target = settings.lan_policy.as_ref().unwrap();
    assert_eq!(target.devices[0].values[&Field::WakeOnLink], None);
    assert_eq!(
        target.devices[0].values[&Field::PnpCapabilities]
            .as_ref()
            .unwrap()
            .kind,
        3
    );
    let directory = tempfile::tempdir().unwrap();
    crate::profiles::save_in(directory.path(), settings.clone(), "网络快照", None).unwrap();
    assert!(
        crate::profiles::load_from(directory.path(), "网络快照")
            .unwrap()
            .settings
            == settings
    );
    assert_eq!(f.lan.io.authorizations.load(Ordering::SeqCst), 0);
    assert!(!f.lan.path().exists());
}
#[test]
fn applying_and_noop_reusing_the_profile_share_one_initial_record_and_do_not_reauthorize() {
    let f = setup();
    let settings = wanted(&f);
    assert!(apply(&f, &settings).iter().all(|step| step.ok));
    assert_eq!(f.lan.io.writes.load(Ordering::SeqCst), 3);
    let before = initial(&f);
    assert_eq!(before.devices[0].fields.len(), 3);
    assert_eq!(before.devices[0].fields[&Field::WakeOnLink], None);
    assert!(
        apply(&f, &settings)
            .iter()
            .any(|step| step.step == "网络驱动策略" && step.ok)
    );
    assert_eq!(f.lan.io.authorizations.load(Ordering::SeqCst), 1);
    assert_eq!(f.lan.io.executions.load(Ordering::SeqCst), 1);
    assert_eq!(f.lan.io.finishes.load(Ordering::SeqCst), 1);
    assert_eq!(initial(&f), before);
}
#[test]
fn a_profile_with_no_network_difference_confirms_current_values_without_creating_a_baseline() {
    let f = setup();
    let settings = ProfileSettings {
        lan_policy: Some(captured(&f)),
        ..Default::default()
    };
    let steps = apply(&f, &settings);
    assert!(
        steps
            .iter()
            .any(|step| step.step == "网络驱动策略" && step.ok)
    );
    assert_eq!(f.lan.io.authorizations.load(Ordering::SeqCst), 0);
    assert!(!f.lan.path().exists());
}
#[test]
fn every_partial_failure_restores_only_attempted_fields_and_does_not_repeat_successful_local_compensation()
 {
    for failure in 1..=3 {
        let f = setup();
        let settings = wanted(&f);
        *f.lan.io.failures.lock().unwrap() = vec![failure];
        let steps = apply(&f, &settings);
        assert!(
            steps
                .iter()
                .any(|step| step.step == "网络驱动策略" && !step.ok)
        );
        assert!(!steps.iter().any(|step| step.step == "恢复网络驱动策略"));
        unchanged(&f);
        assert_eq!(initial(&f).devices[0].fields.len(), failure);
        assert_eq!(f.lan.io.writes.load(Ordering::SeqCst), failure * 2);
        assert_eq!(f.lan.io.authorizations.load(Ordering::SeqCst), 1);
    }
}
#[test]
fn later_gpu_failure_rolls_back_network_with_the_same_authorization_and_keeps_initial_values() {
    let f = setup();
    let mut settings = wanted(&f);
    fail_gpu(&f, &mut settings);
    let steps = apply(&f, &settings);
    assert!(
        steps
            .iter()
            .any(|step| step.step == "恢复网络驱动策略" && step.ok)
    );
    unchanged(&f);
    assert_eq!(f.lan.io.authorizations.load(Ordering::SeqCst), 1);
    assert_eq!(f.lan.io.rollbacks.load(Ordering::SeqCst), 1);
    assert_eq!(
        initial(&f).devices[0].fields[&Field::PowerSavingMode],
        Some(Value::string("0"))
    );
}
#[test]
fn failed_rollback_retains_initial_record_and_independent_restore_can_retry() {
    let f = setup();
    let mut settings = wanted(&f);
    fail_gpu(&f, &mut settings);
    *f.lan.io.refused.lock().unwrap() = vec![4];
    let steps = apply(&f, &settings);
    assert!(
        steps
            .iter()
            .any(|step| step.step == "恢复网络驱动策略" && !step.ok)
    );
    assert!(initial(&f).last_error.is_some());
    let profile = f.lan.controller.read(&f.hal).unwrap().unwrap();
    f.lan
        .controller
        .restore(
            &f.hal,
            &profile.revision,
            &profile.frame.devices[0].identity.instance,
        )
        .unwrap();
    unchanged(&f);
    assert!(initial(&f).last_error.is_none());
}
#[test]
fn single_and_profile_actions_keep_the_earliest_baseline_but_undo_only_the_current_profile() {
    let f = setup();
    let profile = f.lan.controller.read(&f.hal).unwrap().unwrap();
    f.lan
        .controller
        .apply(
            &f.hal,
            &Request {
                expected_revision: profile.revision,
                device_id: profile.frame.devices[0].identity.instance.clone(),
                power_saving: Some(true),
                apply_wake_patch: false,
            },
        )
        .unwrap();
    let mut settings = wanted(&f);
    settings.lan_policy.as_mut().unwrap().devices[0]
        .values
        .insert(Field::PowerSavingMode, Some(Value::string("0")));
    fail_gpu(&f, &mut settings);
    apply(&f, &settings);
    assert_eq!(
        f.lan.io.frame.lock().unwrap().devices[0].power_saving(),
        Some(true)
    );
    assert_eq!(
        initial(&f).devices[0].fields[&Field::PowerSavingMode],
        Some(Value::string("0"))
    );
    let profile = f.lan.controller.read(&f.hal).unwrap().unwrap();
    f.lan
        .controller
        .restore(
            &f.hal,
            &profile.revision,
            &profile.frame.devices[0].identity.instance,
        )
        .unwrap();
    unchanged(&f);
}
#[test]
fn captured_missing_wake_values_are_restored_as_absence_instead_of_kept() {
    let f = setup();
    let target = captured(&f);
    {
        let mut frame = f.lan.io.frame.lock().unwrap();
        frame.devices[0]
            .values
            .insert(Field::PnpCapabilities, Some(Value::dword(256)));
        frame.devices[0]
            .values
            .insert(Field::WakeOnLink, Some(Value::string("2")));
    }
    let settings = ProfileSettings {
        lan_policy: Some(target),
        ..Default::default()
    };
    assert!(apply(&f, &settings).iter().all(|step| step.ok));
    unchanged(&f);
    assert_eq!(
        initial(&f).devices[0].fields[&Field::PnpCapabilities],
        Some(Value::dword(256))
    );
}
#[test]
fn a_later_device_failure_restores_previous_devices_in_one_authorized_batch() {
    let f = setup();
    {
        let mut frame = f.lan.io.frame.lock().unwrap();
        let mut second = frame.devices[0].clone();
        second.identity.instance.push('2');
        second.identity.driver = format!("{CLASS}\\0002");
        frame.devices.push(second);
    }
    let before = f.lan.io.frame.lock().unwrap().devices.clone();
    let settings = wanted(&f);
    *f.lan.io.failures.lock().unwrap() = vec![4];
    assert!(apply(&f, &settings).iter().any(|step| !step.ok));
    assert_eq!(f.lan.io.frame.lock().unwrap().devices, before);
    let record = initial(&f);
    assert_eq!(record.devices.len(), 2);
    assert_eq!(record.devices[0].fields.len(), 3);
    assert_eq!(record.devices[1].fields.len(), 1);
    assert_eq!(f.lan.io.authorizations.load(Ordering::SeqCst), 1);
}
#[test]
fn changed_recovery_ownership_refuses_old_writes_and_preserves_external_file() {
    let f = setup();
    let settings = wanted(&f);
    let path = f.lan.path();
    let external = store::Record {
        last_error: Some("外部记录".into()),
        ..Default::default()
    };
    let copied = external.clone();
    *f.lan.io.after_write.lock().unwrap() = Some(Box::new(move || {
        let current = store::read(&path).unwrap();
        store::write(&path, &current, &copied).unwrap();
    }));
    let steps = apply(&f, &settings);
    assert!(
        steps
            .iter()
            .any(|step| step.step == "恢复网络驱动策略" && !step.ok)
    );
    assert_eq!(initial(&f), external);
    assert_eq!(f.lan.io.writes.load(Ordering::SeqCst), 1);
}
#[test]
fn unattempted_fields_changed_externally_are_not_owned_or_overwritten() {
    let f = setup();
    let settings = wanted(&f);
    let memory = f.lan.io.clone();
    *f.lan.io.after_write.lock().unwrap() = Some(Box::new(move || {
        memory.frame.lock().unwrap().devices[0]
            .values
            .insert(Field::WakeOnLink, Some(Value::string("external")));
    }));
    assert!(apply(&f, &settings).iter().any(|step| !step.ok));
    assert_eq!(
        f.lan.io.frame.lock().unwrap().devices[0].values[&Field::WakeOnLink],
        Some(Value::string("external"))
    );
    assert!(
        !initial(&f).devices[0]
            .fields
            .contains_key(&Field::WakeOnLink)
    );
}
