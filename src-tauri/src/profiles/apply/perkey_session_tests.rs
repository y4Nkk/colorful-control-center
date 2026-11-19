use super::*;

#[test]
fn candidate_prepares_without_hal_and_activates_only_after_full_commit() {
    let h = Harness::new();
    let (gate, runtime, device) = (
        h.f.hal.control_gate(),
        h.runtime.clone(),
        h.f.device.clone(),
    );
    *h.runtime.prepare_hook.lock().unwrap() = Some(Box::new(move || {
        assert!(gate.try_lock().is_ok());
        assert_eq!(runtime.active(), 0);
        assert!(device.0.lock().unwrap().events.is_empty());
    }));
    let steps = h.apply(&h.settings(true)).unwrap();
    assert!(steps.iter().all(|step| step.ok));
    assert_eq!(h.io.active_during_write.load(Ordering::SeqCst), 0);
    assert!(steps.iter().any(|step| step.step == "逐键按键响应"));
    assert_eq!(h.runtime.active(), 1);
    let before = h.io.traffic.lock().unwrap().packets.len();
    h.runtime.event(0x1E, true);
    wait_until(|| h.io.traffic.lock().unwrap().packets.len() > before);
    assert!(
        h.io.traffic
            .lock()
            .unwrap()
            .packets
            .contains(&[8, 0x1E, 10, 20, 30])
    );
    h.apply(&h.settings(false)).unwrap();
    assert_eq!(h.runtime.active(), 0);
    assert_eq!(h.runtime.exited.load(Ordering::SeqCst), 1);
    assert_eq!(h.runtime.exit_with_gate.load(Ordering::SeqCst), 1);
}

#[test]
fn gpu_failure_resumes_only_original_running_session_after_raw_restore() {
    let h = Harness::new();
    h.start_old();
    let before = h.pages();
    h.f.device.0.lock().unwrap().fail_gpu = true;
    let mut wanted = h.settings(true);
    failing_gpu(&mut wanted);
    let steps = h.apply(&wanted).unwrap();
    assert!(
        steps
            .iter()
            .any(|step| step.step == "恢复逐键按键响应" && step.ok)
    );
    assert_eq!(h.pages(), before);
    assert_eq!(h.runtime.active(), 1);
    assert_eq!(h.runtime.exited.load(Ordering::SeqCst), 1);
    assert_eq!(h.runtime.exit_with_gate.load(Ordering::SeqCst), 1);
    assert!(h.f.perkey.read(&h.f.hal).unwrap().reactive_running);
    assert_eq!(h.io.active_during_write.load(Ordering::SeqCst), 0);
}

#[test]
fn local_compensation_success_also_resumes_old_session() {
    let h = Harness::new();
    h.start_old();
    let before = h.pages();
    let next = h.io.traffic.lock().unwrap().packets.len() + 1;
    h.io.traffic.lock().unwrap().fail_sends = vec![next];
    let steps = h.apply(&h.settings(true)).unwrap();
    assert!(steps.iter().any(|step| step.step == "逐键灯效" && !step.ok));
    assert!(
        steps
            .iter()
            .any(|step| step.step == "恢复逐键按键响应" && step.ok)
    );
    assert!(!steps.iter().any(|step| step.step == "恢复逐键灯效"));
    assert_eq!(h.pages(), before);
    assert_eq!(h.runtime.active(), 1);
}

#[test]
fn failed_compensation_keeps_old_paused_and_initial_restore_can_retry() {
    let h = Harness::new();
    h.start_old();
    let next = h.io.traffic.lock().unwrap().packets.len() + 1;
    h.io.traffic.lock().unwrap().fail_sends = (next..next + 3).collect();
    let steps = h.apply(&h.settings(true)).unwrap();
    assert!(
        steps
            .iter()
            .any(|step| step.step == "恢复逐键灯效" && !step.ok)
    );
    assert!(
        steps
            .iter()
            .any(|step| step.step == "恢复逐键按键响应" && !step.ok)
    );
    assert_eq!(h.runtime.active(), 0);
    let status = h.f.perkey.read(&h.f.hal).unwrap();
    assert!(status.can_restore && !status.reactive_running);
    assert!(status.reactive_error.unwrap().contains("保持暂停"));
    h.f.perkey.restore(&h.f.hal).unwrap();
    assert!(!h.f.perkey.read(&h.f.hal).unwrap().can_restore);
    assert_eq!(h.runtime.active(), 0);
}

#[test]
fn already_failed_old_session_is_not_automatically_reactivated_on_rollback() {
    let h = Harness::new();
    h.start_old();
    h.runtime.fail(0);
    wait_until(|| h.runtime.exited.load(Ordering::SeqCst) == 1);
    h.f.device.0.lock().unwrap().fail_gpu = true;
    let mut wanted = h.settings(true);
    failing_gpu(&mut wanted);
    let steps = h.apply(&wanted).unwrap();
    assert!(!steps.iter().any(|step| step.step == "恢复逐键按键响应"));
    assert_eq!(h.runtime.active(), 0);
    assert!(
        h.f.perkey
            .read(&h.f.hal)
            .unwrap()
            .reactive_error
            .unwrap()
            .contains("事件源失效")
    );
}

#[test]
fn preparation_failure_leaves_old_session_and_all_devices_untouched() {
    let h = Harness::new();
    h.start_old();
    let before = h.pages();
    h.runtime.fail_prepare.store(true, Ordering::SeqCst);
    assert!(h.apply(&h.settings(true)).is_err());
    assert_eq!(h.pages(), before);
    assert!(h.f.device.0.lock().unwrap().events.is_empty());
    assert_eq!(h.runtime.active(), 1);
}

#[test]
fn second_preflight_rechecks_cpu_lock_and_drops_candidate_without_pausing_old() {
    let h = Harness::new();
    h.start_old();
    let before = h.pages();
    let device = h.f.device.clone();
    *h.runtime.prepare_hook.lock().unwrap() = Some(Box::new(move || {
        device.0.lock().unwrap().power |= 1 << 63;
    }));
    assert!(h.apply(&h.settings(true)).is_err());
    assert_eq!(h.pages(), before);
    assert!(h.f.device.0.lock().unwrap().events.is_empty());
    assert_eq!(h.runtime.active(), 1);
    assert_eq!(h.runtime.exited.load(Ordering::SeqCst), 1);
    assert_eq!(h.runtime.exit_with_gate.load(Ordering::SeqCst), 1);
}

#[test]
fn second_preflight_rejects_candidate_that_failed_during_preparation() {
    let h = Harness::new();
    h.start_old();
    let runtime = h.runtime.clone();
    *h.runtime.prepare_hook.lock().unwrap() = Some(Box::new(move || runtime.fail(1)));
    assert!(h.apply(&h.settings(true)).is_err());
    assert!(h.f.device.0.lock().unwrap().events.is_empty());
    assert_eq!(h.runtime.active(), 1);
    assert_eq!(h.runtime.exited.load(Ordering::SeqCst), 1);
}

#[test]
fn second_preflight_rechecks_full_layout_and_preserves_first_baseline_binding() {
    let h = Harness::new();
    let io = h.io.clone();
    *h.runtime.prepare_hook.lock().unwrap() = Some(Box::new(move || {
        io.info.lock().unwrap().compact = Some(true);
    }));
    assert!(h.apply(&h.settings(true)).is_err());
    assert!(h.f.device.0.lock().unwrap().events.is_empty());
    h.io.info.lock().unwrap().compact = Some(false);
    h.apply(&h.settings(false)).unwrap();
    h.io.info.lock().unwrap().compact = Some(true);
    assert!(h.f.perkey.restore(&h.f.hal).is_err());
    h.io.info.lock().unwrap().compact = Some(false);
    h.f.perkey.restore(&h.f.hal).unwrap();
}

#[test]
fn file_changed_while_preparing_rejects_manual_confirmation_without_writes() {
    let h = Harness::new();
    h.start_old();
    let dir = tempfile::tempdir().unwrap();
    crate::profiles::save_in(dir.path(), h.settings(true), "逐键", None).unwrap();
    let expected = crate::profiles::load_from(dir.path(), "逐键").unwrap();
    let mut changed = expected.clone();
    changed.saved_at_ms += 1;
    let path = dir.path().join("逐键.json");
    *h.runtime.prepare_hook.lock().unwrap() = Some(Box::new(move || {
        std::fs::write(path, serde_json::to_vec(&changed).unwrap()).unwrap();
    }));
    let result = transact(
        &h.f.hal,
        &h.f.gpu,
        h.f.devices(),
        &expected.settings,
        ModeReview::Confirmed {
            permission: None,
            mode: None,
            file: Some((dir.path(), &expected)),
            display: None,
            gpu_power: None,
            graphics_mode: None,
        },
    );
    assert!(result.err().unwrap().to_string().contains("档案已变化"));
    assert!(h.f.device.0.lock().unwrap().events.is_empty());
    assert_eq!(h.runtime.active(), 1);
    assert_eq!(h.runtime.exited.load(Ordering::SeqCst), 1);
}

#[test]
fn profile_single_perkey_and_options_operations_share_one_lock_order() {
    let h = Harness::new();
    let wanted = h.settings(true);
    std::thread::scope(|scope| {
        let applying = scope.spawn(|| h.apply(&wanted).unwrap());
        let reading = scope.spawn(|| {
            h.f.perkey.read(&h.f.hal).unwrap();
            h.f.lighting_options.read(&h.f.hal).unwrap();
        });
        assert!(applying.join().unwrap().iter().all(|step| step.ok));
        reading.join().unwrap();
    });
    assert_eq!(h.runtime.active(), 1);
    h.f.perkey.restore(&h.f.hal).unwrap();
    assert_eq!(h.runtime.active(), 0);
}

#[test]
fn standalone_restore_failure_retains_baseline_and_original_snapshot_for_retry() {
    let h = Harness::new();
    let before = h.pages();
    h.apply(&h.settings(false)).unwrap();
    let next = h.io.traffic.lock().unwrap().packets.len() + 1;
    h.io.traffic.lock().unwrap().fail_sends = vec![next];
    assert!(h.f.perkey.restore(&h.f.hal).is_err());
    assert!(h.f.perkey.read(&h.f.hal).unwrap().can_restore);
    h.f.perkey.restore(&h.f.hal).unwrap();
    assert_eq!(h.pages(), before);
    assert!(!h.f.perkey.read(&h.f.hal).unwrap().can_restore);
}
