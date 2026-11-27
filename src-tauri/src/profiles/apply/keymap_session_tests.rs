use super::*;

#[test]
fn later_failure_restores_exact_old_record_and_resumes_only_original_session() {
    let h = Harness::new();
    h.start_old();
    let record = h.record();
    let mut wanted = h.wanted(true);
    gpu_failure(&h, &mut wanted);
    wanted.keymap.as_mut().unwrap().configuration.devices[0].bindings[0].trigger = 32;
    let steps = h.apply(&wanted).unwrap();
    assert!(
        steps
            .iter()
            .any(|step| step.step == "恢复程序宏监听" && step.ok)
    );
    assert_eq!(h.record(), record);
    assert_eq!(h.programs.active(), 1);
    assert_eq!(h.programs.exited.load(Ordering::SeqCst), 1);
    wait_until(|| h.running());
    assert_eq!(h.programs.cutovers(), 2);
    h.programs.event(256);
    wait_until(|| h.programs.launched.lock().unwrap().len() == 1);
}

#[test]
fn failed_original_session_stays_failed_after_later_transaction_rollback() {
    let h = Harness::new();
    h.start_old();
    h.programs.fail(0);
    wait_until(|| h.programs.exited.load(Ordering::SeqCst) == 1);
    let mut wanted = h.wanted(true);
    gpu_failure(&h, &mut wanted);
    let steps = h.apply(&wanted).unwrap();
    assert!(!steps.iter().any(|step| step.step == "恢复程序宏监听"));
    assert_eq!(h.programs.active(), 0);
    assert!(
        h.profile()["programs"]["error"]
            .as_str()
            .unwrap()
            .contains("事件源失效")
    );
}

#[test]
fn preparation_failure_preserves_old_listener_record_and_has_zero_target_writes() {
    let h = Harness::new();
    h.start_old();
    let record = h.record();
    h.programs.fail_prepare.store(true, Ordering::SeqCst);
    assert!(h.apply(&h.wanted(true)).is_err());
    assert_eq!(h.record(), record);
    assert!(h.f.device.0.lock().unwrap().events.is_empty());
    assert_eq!(h.programs.active(), 1);
}

#[test]
fn second_preflight_rechecks_source_fingerprint_before_cpu_or_sdk_write() {
    let h = Harness::new();
    let io = h.io.clone();
    *h.programs.prepare_hook.lock().unwrap() = Some(Box::new(move || {
        io.source.lock().unwrap().as_mut().unwrap().fingerprint = "new-full-source".into();
    }));
    assert!(h.apply(&h.wanted(true)).is_err());
    assert!(!h.path.exists());
    assert!(h.f.device.0.lock().unwrap().events.is_empty());
    assert_eq!(h.programs.exited.load(Ordering::SeqCst), 1);
    assert_eq!(h.programs.exited_without_gate.load(Ordering::SeqCst), 1);
}

#[test]
fn second_preflight_rechecks_other_domains_after_program_preparation() {
    let h = Harness::new();
    h.start_old();
    let device = h.f.device.clone();
    let record = h.record();
    let mut wanted = h.wanted(true);
    wanted.runtime_cpu.as_mut().unwrap().pl1_w = Some(75.0);
    *h.programs.prepare_hook.lock().unwrap() = Some(Box::new(move || {
        device.0.lock().unwrap().power |= 1 << 63;
    }));
    assert!(h.apply(&wanted).is_err());
    assert_eq!(h.record(), record);
    assert!(h.f.device.0.lock().unwrap().events.is_empty());
    assert_eq!(h.programs.active(), 1);
}

#[test]
fn local_partial_failure_compensates_and_resumes_old_listener_without_outer_duplicate() {
    let h = Harness::new();
    h.start_old();
    let record = h.record();
    h.io.fail_flags.store(1, Ordering::SeqCst);
    let steps = h.apply(&h.wanted(true)).unwrap();
    assert!(
        steps
            .iter()
            .any(|step| step.step == "按键映射与宏" && !step.ok)
    );
    assert!(!steps.iter().any(|step| step.step == "恢复按键映射与宏"));
    assert!(
        steps
            .iter()
            .any(|step| step.step == "恢复程序宏监听" && step.ok)
    );
    assert_eq!(h.record(), record);
    assert_eq!(h.programs.active(), 1);
}

#[test]
fn failed_compensation_retains_prepared_journal_and_explicit_initial_restore_can_retry() {
    let h = Harness::new();
    h.start_old();
    h.io.fail_replace.store(3, Ordering::SeqCst);
    let steps = h.apply(&h.wanted(true)).unwrap();
    assert!(
        steps
            .iter()
            .any(|step| step.step == "恢复按键映射与宏" && !step.ok)
    );
    assert!(
        steps
            .iter()
            .any(|step| step.step == "恢复程序宏监听" && !step.ok)
    );
    assert_eq!(h.record()["phase"], "prepared");
    assert_eq!(h.programs.active(), 0);
    h.f.keymap.restore(&h.f.hal, &h.revision()).unwrap();
    assert_eq!(*h.io.current.lock().unwrap(), original());
    assert_eq!(h.record()["phase"], "applied");
    assert_eq!(h.programs.active(), 0);
}

#[test]
fn explicit_disabled_program_policy_keeps_macros_saved_and_stops_listener_after_commit() {
    let h = Harness::new();
    h.start_old();
    assert!(
        h.apply(&h.wanted(false))
            .unwrap()
            .iter()
            .all(|step| step.ok)
    );
    assert_eq!(h.programs.active(), 0);
    assert!(!h.running());
    assert_eq!(
        h.record()["current"]["devices"][0]["bindings"][0]["action"]["kind"],
        "launch"
    );
    assert_eq!(h.programs.exited.load(Ordering::SeqCst), 1);
    assert_eq!(h.programs.exited_without_gate.load(Ordering::SeqCst), 1);
}

#[test]
fn macro_events_wait_for_control_gate_and_recording_suppresses_launches() {
    let h = Harness::new();
    h.start_old();
    {
        let _gate = h.f.hal.begin_write().unwrap();
        h.programs.event(256);
        std::thread::sleep(std::time::Duration::from_millis(50));
        assert!(h.programs.launched.lock().unwrap().is_empty());
    }
    wait_until(|| h.programs.launched.lock().unwrap().len() == 1);
    h.programs.recording.store(true, Ordering::SeqCst);
    h.programs.event(256);
    std::thread::sleep(std::time::Duration::from_millis(60));
    assert_eq!(h.programs.launched.lock().unwrap().len(), 1);
}

#[test]
fn mixed_profile_and_single_mapping_share_first_persistent_baseline_and_lock_order() {
    let h = Harness::new();
    std::thread::scope(|scope| {
        let apply = scope.spawn(|| h.apply(&h.wanted(false)).unwrap());
        let read = scope.spawn(|| h.f.keymap.read(&h.f.hal).unwrap());
        assert!(apply.join().unwrap().iter().all(|step| step.ok));
        read.join().unwrap();
    });
    let before = h.record()["baseline"].clone();
    let target = h.wanted(false).keymap.unwrap().configuration;
    h.f.keymap
        .apply(
            &h.f.hal,
            &crate::keymap::Request {
                expected_revision: h.revision(),
                configuration: target,
            },
        )
        .unwrap();
    assert_eq!(h.record()["baseline"], before);
    h.f.keymap.restore(&h.f.hal, &h.revision()).unwrap();
    assert_eq!(*h.io.current.lock().unwrap(), original());
}

#[test]
fn changed_file_and_failed_candidate_reject_before_any_target_writes() {
    for failed_candidate in [false, true] {
        let h = Harness::new();
        let dir = tempfile::tempdir().unwrap();
        crate::profiles::save_in(dir.path(), h.wanted(true), "映射", None).unwrap();
        let expected = crate::profiles::load_from(dir.path(), "映射").unwrap();
        let mut changed = expected.clone();
        changed.saved_at_ms += 1;
        let path = dir.path().join("映射.json");
        let programs = h.programs.clone();
        *h.programs.prepare_hook.lock().unwrap() = Some(Box::new(move || {
            if failed_candidate {
                programs.fail(0);
            } else {
                std::fs::write(path, serde_json::to_vec(&changed).unwrap()).unwrap();
            }
        }));
        let result = transact(
            &h.f.hal,
            &h.f.gpu,
            h.f.devices(),
            &expected.settings,
            ModeReview::Confirmed {
                permission: None,
                mode: None,
                source: Some((&crate::profiles::application::Source::Saved(dir.path().into()), &expected)),
                display: None,
                gpu_power: None,
                graphics_mode: None,
            },
        );
        assert!(result.is_err());
        assert!(!h.path.exists());
        assert!(h.f.device.0.lock().unwrap().events.is_empty());
        assert_eq!(h.programs.exited.load(Ordering::SeqCst), 1);
    }
}

#[test]
fn final_commit_failure_keeps_listener_paused_and_recovery_journal_retryable() {
    let h = Harness::new();
    h.start_old();
    let before = h.f.device.0.lock().unwrap().power;
    let mut wanted = h.wanted(true);
    wanted.runtime_cpu.as_mut().unwrap().pl1_w = Some(75.0);
    wanted.keymap.as_mut().unwrap().configuration.devices[0].bindings[0].trigger = 32;
    h.io.lock_commit.store(true, Ordering::SeqCst);
    let steps = h.apply(&wanted).unwrap();
    assert!(
        steps
            .iter()
            .any(|step| step.step == "提交映射与程序监听" && !step.ok)
    );
    assert!(
        steps
            .iter()
            .any(|step| step.step == "恢复按键映射与宏" && !step.ok)
    );
    assert_eq!(h.programs.active(), 0);
    assert_eq!(h.f.device.0.lock().unwrap().power, before);
    h.io.lock_commit.store(false, Ordering::SeqCst);
    h.io.commit_lock.lock().unwrap().take();
    assert_eq!(h.record()["phase"], "prepared");
    h.f.keymap.restore(&h.f.hal, &h.revision()).unwrap();
    assert_eq!(h.record()["phase"], "applied");
    assert_eq!(*h.io.current.lock().unwrap(), original());
}

#[test]
fn unknown_pending_before_state_is_never_replayed_as_a_successful_compensation() {
    let h = Harness::new();
    h.io.fail_replace.store(3, Ordering::SeqCst);
    h.apply(&h.wanted(true)).unwrap();
    assert_eq!(h.record()["phase"], "prepared");
    let before =
        h.f.device
            .0
            .lock()
            .unwrap()
            .events
            .iter()
            .filter(|event| event.as_str() == "KEYMAP:replace")
            .count();
    h.io.fail_replace.store(1, Ordering::SeqCst);
    let error =
        h.f.keymap
            .restore(&h.f.hal, &h.revision())
            .err()
            .unwrap()
            .to_string();
    assert!(error.contains("先前映射执行状态未知"));
    let after =
        h.f.device
            .0
            .lock()
            .unwrap()
            .events
            .iter()
            .filter(|event| event.as_str() == "KEYMAP:replace")
            .count();
    assert_eq!(after, before + 1);
    assert_eq!(h.record()["phase"], "prepared");
    h.f.keymap.restore(&h.f.hal, &h.revision()).unwrap();
    assert_eq!(*h.io.current.lock().unwrap(), original());
}

#[test]
fn partial_profile_cannot_enable_programs_on_a_device_missing_from_its_preview() {
    use ccc_core::keymap::{Configuration, Device, DeviceSettings};
    let h = Harness::new();
    h.start_old();
    let record = h.record();
    let mut wanted = h.wanted(true);
    wanted.keymap.as_mut().unwrap().configuration = Configuration {
        devices: vec![DeviceSettings {
            device: Device::Mouse,
            enabled: true,
            bindings: vec![],
        }],
    };
    let error = h.apply(&wanted).err().unwrap().to_string();
    assert!(error.contains("档案未展示的已接管设备"));
    assert_eq!(h.record(), record);
    assert!(h.f.device.0.lock().unwrap().events.is_empty());
    assert_eq!(h.programs.active(), 1);
    h.f.keymap
        .apply(
            &h.f.hal,
            &crate::keymap::Request {
                expected_revision: h.revision(),
                configuration: wanted.keymap.unwrap().configuration,
            },
        )
        .unwrap();
    let captured = crate::profiles::capture_current(&h.f.hal, &h.f.gpu, h.f.devices(), &crate::profiles::ProfileGroup::ALL).unwrap();
    assert_eq!(captured.keymap.unwrap().configuration.devices.len(), 2);
}
