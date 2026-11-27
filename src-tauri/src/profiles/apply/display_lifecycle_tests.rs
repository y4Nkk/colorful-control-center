use super::*;

#[test]
fn background_busy_query_observes_expired_display_trials_without_ui_polling() {
    let h = Harness::new();
    h.begin();
    assert!(h.application.is_busy(&h.f.display.controller).unwrap());
    h.f.display.controller.tick(u64::MAX).unwrap();
    assert!(!h.application.is_busy(&h.f.display.controller).unwrap());
    h.no_other_writes();
    h.restored();
}

#[test]
fn startup_origin_survives_explicit_keep_and_does_not_become_manual_application() {
    let h = Harness::new();
    assert!(!h.application.is_busy(&h.f.display.controller).unwrap());
    let mut runtime = h.runtime();
    runtime.origin = crate::profiles::application::Origin::Startup;
    runtime.permission = Some(crate::profiles::application::Permission::new(|| Ok(())));
    let result = h
        .application
        .begin(runtime, &h.file, None, Some(&h.review), None, None)
        .unwrap();
    assert!(result.origin == crate::profiles::application::Origin::Startup);
    assert!(h.application.is_busy(&h.f.display.controller).unwrap());
    h.no_other_writes();
    let result = h
        .application
        .keep(&h.f.hal, &h.f.gpu, h.f.devices(), result.id)
        .unwrap();
    assert!(result.origin == crate::profiles::application::Origin::Startup);
    assert!(result.pending.is_none());
    assert!(!h.application.is_busy(&h.f.display.controller).unwrap());
}

#[test]
fn authorization_file_change_after_keep_prevents_all_remaining_outputs_and_restores_display() {
    use std::sync::atomic::Ordering;
    let mut h = Harness::new();
    h.enable_lan();
    let id = h.begin().id;
    let path = h.dir.path().join("显示组合.json");
    let mut changed = h.file.clone();
    changed.settings.pl1_w = Some(88.0);
    *h.f.lan.io.on_authorize.lock().unwrap() = Some(Box::new(move || {
        std::fs::write(path, serde_json::to_vec(&changed).unwrap()).unwrap();
    }));
    let result = h
        .application
        .keep(&h.f.hal, &h.f.gpu, h.f.devices(), id)
        .unwrap();
    assert!(
        result
            .steps
            .iter()
            .any(|step| !step.ok && step.message.contains("档案已变化"))
    );
    assert!(result.pending.is_none());
    assert_eq!(h.f.lan.io.authorizations.load(Ordering::SeqCst), 1);
    assert_eq!(h.f.lan.io.executions.load(Ordering::SeqCst), 0);
    h.no_other_writes();
    h.restored();
}

#[test]
fn authorization_display_change_after_keep_stops_other_outputs_and_preserves_external_mode() {
    use std::sync::atomic::Ordering;
    let mut h = Harness::new();
    h.enable_lan();
    let id = h.begin().id;
    let displays = h.f.display.io.0.clone();
    *h.f.lan.io.on_authorize.lock().unwrap() = Some(Box::new(move || {
        displays.lock().unwrap().displays[1].current.frequency = 120;
    }));
    let result = h
        .application
        .keep(&h.f.hal, &h.f.gpu, h.f.devices(), id)
        .unwrap();
    assert!(
        result
            .steps
            .iter()
            .any(|step| !step.ok && step.message.contains("显示模式已变化"))
    );
    assert!(result.pending.is_some());
    assert_eq!(h.f.lan.io.executions.load(Ordering::SeqCst), 0);
    assert_eq!(
        h.f.display.io.0.lock().unwrap().displays[1]
            .current
            .frequency,
        120
    );
    h.no_other_writes();
    h.f.display.io.0.lock().unwrap().displays[1]
        .current
        .frequency = 144;
    h.application
        .cancel(&h.f.hal, &h.f.display.controller, id)
        .unwrap();
    h.restored();
}

#[test]
fn completed_startup_result_survives_a_later_manual_application() {
    let h = Harness::new();
    let mut runtime = h.runtime();
    runtime.origin = crate::profiles::application::Origin::Startup;
    runtime.permission = Some(crate::profiles::application::Permission::new(|| Ok(())));
    let initial = h
        .application
        .begin(runtime, &h.file, None, Some(&h.review), None, None)
        .unwrap();
    let completed = h
        .application
        .keep(&h.f.hal, &h.f.gpu, h.f.devices(), initial.id)
        .unwrap();
    assert!(completed.revision > initial.revision);
    let current =
        h.f.display
            .controller
            .preview_profile(h.file.settings.display.as_ref().unwrap(), None)
            .unwrap();
    let manual = h
        .application
        .begin(h.runtime(), &h.file, None, Some(&current), None, None)
        .unwrap();
    assert!(manual.origin != crate::profiles::application::Origin::Startup);
    let startup = h
        .application
        .startup_report(&h.f.display.controller)
        .unwrap()
        .unwrap();
    assert_eq!(
        (startup.id, startup.revision),
        (completed.id, completed.revision)
    );
    assert!(startup.pending.is_none());
    assert!(startup.steps.iter().all(|step| step.ok));
}

#[test]
fn timed_out_startup_is_resolved_before_a_manual_application_replaces_current_report() {
    let h = Harness::new();
    let mut runtime = h.runtime();
    runtime.origin = crate::profiles::application::Origin::Startup;
    runtime.permission = Some(crate::profiles::application::Permission::new(|| Ok(())));
    let initial = h
        .application
        .begin(runtime, &h.file, None, Some(&h.review), None, None)
        .unwrap();
    h.f.display.controller.tick(u64::MAX).unwrap();
    let manual = h.begin();
    let startup = h
        .application
        .startup_report(&h.f.display.controller)
        .unwrap()
        .unwrap();
    assert_eq!(startup.id, initial.id);
    assert!(startup.revision > initial.revision);
    assert!(startup.pending.is_none());
    assert!(
        startup
            .steps
            .iter()
            .any(|step| !step.ok && step.message.contains("未执行"))
    );
    h.application
        .cancel(&h.f.hal, &h.f.display.controller, manual.id)
        .unwrap();
    h.no_other_writes();
}

#[test]
fn automatic_display_recovery_error_is_visible_and_blocks_keep_until_recovery_succeeds() {
    let h = Harness::new();
    let initial = h.begin();
    h.f.display.io.0.lock().unwrap().fail_at = vec![3];
    assert!(h.f.display.controller.tick(u64::MAX).is_err());
    let report = h
        .application
        .read(&h.f.display.controller)
        .unwrap()
        .unwrap();
    assert!(report.revision > initial.revision);
    assert!(report.pending.unwrap().error.unwrap().contains("恢复失败"));
    assert!(
        h.application
            .keep(&h.f.hal, &h.f.gpu, h.f.devices(), initial.id)
            .is_err()
    );
    h.no_other_writes();
    h.f.display.io.0.lock().unwrap().fail_at.clear();
    let completed = h
        .application
        .cancel(&h.f.hal, &h.f.display.controller, initial.id)
        .unwrap();
    assert!(completed.pending.is_none());
    assert!(completed.revision > report.revision);
    h.restored();
}

#[test]
fn authorization_cleanup_issue_keeps_confirmed_display_and_all_other_profile_targets() {
    use std::sync::atomic::Ordering;
    let mut h = Harness::new();
    h.enable_lan();
    let id = h.begin().id;
    h.f.lan.io.fail_finish.store(true, Ordering::SeqCst);
    let report = h
        .application
        .keep(&h.f.hal, &h.f.gpu, h.f.devices(), id)
        .unwrap();
    assert!(report.targets_applied);
    assert!(report.pending.is_none());
    assert!(
        report
            .steps
            .iter()
            .any(|step| step.step == "网络授权收尾" && !step.ok)
    );
    assert!(
        report
            .steps
            .iter()
            .any(|step| step.step == "保留档案显示" && step.ok)
    );
    assert!(
        !report
            .steps
            .iter()
            .any(|step| step.step == "恢复本次显示变更")
    );
    assert!(
        h.f.display
            .io
            .0
            .lock()
            .unwrap()
            .displays
            .iter()
            .all(|display| display.current.frequency == 144)
    );
    assert_eq!(
        &h.f.device.0.lock().unwrap().pages[6][33..37],
        &77i32.to_le_bytes()
    );
    assert_eq!(
        h.f.lan.io.frame.lock().unwrap().devices[0].power_saving(),
        Some(true)
    );
}
