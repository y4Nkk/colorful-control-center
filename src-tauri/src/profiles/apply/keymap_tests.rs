use super::*;
use std::sync::atomic::Ordering;
#[path = "keymap_fixture.rs"]
mod fixture;
use fixture::*;
#[path = "keymap_combination_tests.rs"]
mod combination_tests;
#[path = "keymap_session_tests.rs"]
mod session_tests;

#[test]
fn capture_keeps_only_owned_devices_current_flags_and_program_intent_without_starting_events() {
    let h = Harness::new();
    let captured = crate::profiles::capture_current(&h.f.hal, &h.f.gpu, h.f.devices()).unwrap();
    assert_eq!(captured.keymap.unwrap().configuration.devices.len(), 2);
    assert!(!h.path.exists());
    assert_eq!(h.programs.prepared.load(Ordering::SeqCst), 0);
    h.apply(&h.wanted(false)).unwrap();
    h.f.device.0.lock().unwrap().pages[1][8] = 0;
    let captured = crate::profiles::capture_current(&h.f.hal, &h.f.gpu, h.f.devices()).unwrap();
    let target = captured.keymap.as_ref().unwrap();
    assert_eq!(target.configuration.devices.len(), 1);
    assert!(!target.configuration.devices[0].enabled && !target.programs_enabled);
    let dir = tempfile::tempdir().unwrap();
    crate::profiles::save_in(dir.path(), captured.clone(), "映射").unwrap();
    assert!(
        crate::profiles::load_from(dir.path(), "映射")
            .unwrap()
            .settings
            == captured
    );
    assert_eq!(h.programs.active(), 0);
}

#[test]
fn strict_targets_and_missing_source_or_owner_fail_before_any_cpu_or_mapping_write() {
    let h = Harness::new();
    let mut invalid = h.wanted(false);
    invalid
        .keymap
        .as_mut()
        .unwrap()
        .configuration
        .devices
        .clear();
    assert!(h.apply(&invalid).is_err());
    h.io.blocked.store(true, Ordering::SeqCst);
    assert!(h.apply(&h.wanted(false)).is_err());
    h.io.blocked.store(false, Ordering::SeqCst);
    h.io.source.lock().unwrap().take();
    assert!(h.apply(&h.wanted(false)).is_err());
    assert!(!h.path.exists());
    assert!(h.f.device.0.lock().unwrap().events.is_empty());
    let mut json = serde_json::to_value(h.wanted(false).keymap.unwrap()).unwrap();
    json["obsolete"] = serde_json::json!(true);
    assert!(serde_json::from_value::<crate::keymap::Target>(json).is_err());
    assert!(
        serde_json::from_value::<crate::keymap::Target>(
            serde_json::json!({"configuration":original()})
        )
        .is_err()
    );
}

#[test]
fn gpu_failure_rolls_back_mapping_flags_and_cpu_before_any_program_commit() {
    let h = Harness::new();
    let before = h.f.device.0.lock().unwrap().power;
    let mut wanted = h.wanted(true);
    gpu_failure(&h, &mut wanted);
    let steps = h.apply(&wanted).unwrap();
    assert!(
        steps
            .iter()
            .any(|step| step.step == "按键映射与宏" && step.ok)
    );
    assert!(
        steps
            .iter()
            .any(|step| step.step == "恢复按键映射与宏" && step.ok)
    );
    assert!(!steps.iter().any(|step| step.step == "提交映射与程序监听"));
    assert_eq!(*h.io.current.lock().unwrap(), original());
    assert_eq!(h.f.device.0.lock().unwrap().power, before);
    assert_eq!(h.record()["phase"], "applied");
    assert_eq!(h.record()["baseline"]["flags"], serde_json::json!([0, 1]));
    assert_eq!(h.programs.active(), 0);
    assert_eq!(h.programs.exited.load(Ordering::SeqCst), 1);
    assert_eq!(h.programs.exited_without_gate.load(Ordering::SeqCst), 1);
}

#[test]
fn candidate_is_prepared_outside_gate_and_discards_precommit_keys_before_launching() {
    let h = Harness::new();
    let gate = h.f.hal.control_gate();
    let programs = h.programs.clone();
    let path = h.path.clone();
    *h.programs.prepare_hook.lock().unwrap() = Some(Box::new(move || {
        assert!(gate.try_lock().is_ok());
        assert!(!path.exists());
        assert_eq!(programs.active(), 0);
        programs.event(256);
    }));
    let steps = h.apply(&h.wanted(true)).unwrap();
    assert!(steps.iter().all(|step| step.ok));
    wait_until(|| h.running());
    assert!(h.programs.launched.lock().unwrap().is_empty());
    assert_eq!(h.record()["phase"], "applied");
    h.programs.event(256);
    wait_until(|| h.programs.launched.lock().unwrap().len() == 1);
    assert_eq!(
        h.programs.launched.lock().unwrap()[0],
        r"F:\isolated\helper.exe"
    );
}
