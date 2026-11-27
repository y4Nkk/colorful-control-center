use super::*;
use crate::profiles::application::{Origin, Permission};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[test]
fn builtin_source_rechecks_power_after_display_trial_and_restores_on_disconnect() {
    use crate::profiles::{application::Source, apply::review::Reviewed};

    let mut h = Harness::new();
    h.file.name = "插电游戏".into();
    let connected = Arc::new(AtomicBool::new(false));
    let current = connected.clone();
    let source = Source::BuiltIn(Arc::new(move || {
        anyhow::ensure!(current.load(Ordering::SeqCst), "模拟电源已拔出");
        Ok(())
    }));
    let begin = || {
        h.application
            .begin_reviewed(
                Origin::Manual,
                &h.f.hal,
                &h.f.gpu,
                h.f.devices(),
                Reviewed {
                    permission: None,
                    source: &source,
                    file: &h.file,
                    mode: None,
                    display: Some(&h.review),
                    gpu_power: None,
                    graphics_mode: None,
                },
            )
            .unwrap()
    };
    let rejected = begin();
    assert!(rejected.pending.is_none() && !rejected.targets_applied);
    assert!(h.f.display.io.0.lock().unwrap().writes.is_empty());
    h.no_other_writes();

    connected.store(true, Ordering::SeqCst);
    let trial = begin();
    assert!(trial.pending.is_some());
    h.no_other_writes();
    connected.store(false, Ordering::SeqCst);
    let report = h
        .application
        .keep(&h.f.hal, &h.f.gpu, h.f.devices(), trial.id)
        .unwrap();
    assert!(report.pending.is_none() && !report.targets_applied);
    assert!(
        report
            .steps
            .iter()
            .any(|step| !step.ok && step.message.contains("模拟电源已拔出"))
    );
    h.restored();
    h.no_other_writes();
}

fn permission(enabled: &Arc<AtomicBool>) -> Permission {
    let enabled = enabled.clone();
    Permission::new(move || {
        anyhow::ensure!(enabled.load(Ordering::SeqCst), "自动档案授权已撤销");
        Ok(())
    })
}

#[test]
fn background_system_permission_is_checked_before_display_trial() {
    for origin in [Origin::Startup, Origin::Ac, Origin::Battery, Origin::Resume] {
        let mut h = Harness::new();
        h.enable_lan();
        let mut runtime = h.runtime();
        runtime.origin = origin;
        runtime.permission = Some(Permission::with_mode_driver(
            || Ok(()),
            |_| Ok(()),
            || anyhow::bail!("isolated token is not elevated"),
        ));
        let result = h
            .application
            .begin(runtime, &h.file, None, Some(&h.review), None, None);
        let report = result.unwrap();
        assert!(!report.targets_applied && report.pending.is_none());
        assert!(
            report
                .steps
                .iter()
                .any(|step| !step.ok && step.message.contains("not elevated"))
        );
        assert_eq!(h.f.lan.io.authorizations.load(Ordering::SeqCst), 0);
        assert_eq!(h.f.lan.io.writes.load(Ordering::SeqCst), 0);
        h.no_other_writes();
        assert!(h.f.display.io.0.lock().unwrap().writes.is_empty());
    }
}

#[test]
fn ordinary_background_profile_does_not_require_system_permission() {
    let h = Harness::new();
    let mut runtime = h.runtime();
    runtime.origin = Origin::Startup;
    runtime.permission = Some(Permission::with_mode_driver(
        || Ok(()),
        |_| Ok(()),
        || anyhow::bail!("must not query elevation without privileged changes"),
    ));
    let report = h
        .application
        .begin(runtime, &h.file, None, Some(&h.review), None, None)
        .unwrap();
    assert!(report.pending.is_some());
    h.no_other_writes();
    h.application
        .cancel(&h.f.hal, &h.f.display.controller, report.id)
        .unwrap();
    h.restored();
}

#[test]
fn origin_serialization_uses_one_current_contract_without_startup_flag() {
    for (origin, value) in [
        (Origin::Manual, "manual"),
        (Origin::Startup, "startup"),
        (Origin::Ac, "ac"),
        (Origin::Battery, "battery"),
        (Origin::Resume, "resume"),
    ] {
        assert_eq!(serde_json::to_value(origin).unwrap(), value);
    }
    let h = Harness::new();
    let report = h.begin();
    let value = serde_json::to_value(report).unwrap();
    assert_eq!(value["origin"], "manual");
    assert!(value.get("startup").is_none());
}

#[test]
fn automatic_origin_requires_permission_and_manual_origin_cannot_inherit_one() {
    let h = Harness::new();
    for origin in [Origin::Startup, Origin::Ac, Origin::Battery, Origin::Resume] {
        let mut runtime = h.runtime();
        runtime.origin = origin;
        assert!(
            h.application
                .begin(runtime, &h.file, None, Some(&h.review), None, None)
                .is_err()
        );
    }
    for origin in [Origin::Manual] {
        let mut runtime = h.runtime();
        runtime.origin = origin;
        runtime.permission = Some(Permission::new(|| Ok(())));
        assert!(
            h.application
                .begin(runtime, &h.file, None, Some(&h.review), None, None)
                .is_err()
        );
    }
    h.no_other_writes();
    assert!(h.f.display.io.0.lock().unwrap().writes.is_empty());
}

#[test]
fn revoked_before_begin_has_no_display_or_other_output() {
    let h = Harness::new();
    let enabled = Arc::new(AtomicBool::new(false));
    let mut runtime = h.runtime();
    runtime.origin = Origin::Ac;
    runtime.permission = Some(permission(&enabled));
    assert!(
        h.application
            .begin(runtime, &h.file, None, Some(&h.review), None, None)
            .is_err()
    );
    assert!(h.f.display.io.0.lock().unwrap().writes.is_empty());
    h.no_other_writes();
    assert!(
        h.application
            .read(&h.f.display.controller)
            .unwrap()
            .is_none()
    );
}

#[test]
fn manual_keep_cannot_discard_automatic_permission_and_recovery_is_allowed() {
    let h = Harness::new();
    let enabled = Arc::new(AtomicBool::new(true));
    let mut runtime = h.runtime();
    runtime.origin = Origin::Battery;
    runtime.permission = Some(permission(&enabled));
    let report = h
        .application
        .begin(runtime, &h.file, None, Some(&h.review), None, None)
        .unwrap();
    enabled.store(false, Ordering::SeqCst);
    let report = h
        .application
        .keep(&h.f.hal, &h.f.gpu, h.f.devices(), report.id)
        .unwrap();
    assert_eq!(report.origin, Origin::Battery);
    assert!(!report.targets_applied && report.pending.is_none());
    assert!(
        report
            .steps
            .iter()
            .any(|s| !s.ok && s.message.contains("授权已撤销"))
    );
    h.no_other_writes();
    h.restored();
}

#[test]
fn revoked_while_waiting_for_system_authorization_has_no_remaining_output() {
    let mut h = Harness::new();
    h.enable_lan();
    let enabled = Arc::new(AtomicBool::new(true));
    let mut runtime = h.runtime();
    runtime.origin = Origin::Resume;
    runtime.permission = Some(permission(&enabled));
    let report = h
        .application
        .begin(runtime, &h.file, None, Some(&h.review), None, None)
        .unwrap();
    let grant = enabled.clone();
    *h.f.lan.io.on_authorize.lock().unwrap() =
        Some(Box::new(move || grant.store(false, Ordering::SeqCst)));
    let report = h
        .application
        .keep(&h.f.hal, &h.f.gpu, h.f.devices(), report.id)
        .unwrap();
    assert!(!report.targets_applied && report.pending.is_none());
    assert!(
        report
            .steps
            .iter()
            .any(|s| !s.ok && s.message.contains("授权已撤销"))
    );
    assert_eq!(h.f.lan.io.executions.load(Ordering::SeqCst), 0);
    h.no_other_writes();
    h.restored();
}

#[test]
fn revocation_after_a_device_transaction_rolls_it_back_without_blocking_recovery() {
    let h = Harness::new();
    let initial = h.f.device.0.lock().unwrap().pages[6][33..37].to_vec();
    let device = h.f.device.clone();
    let mut runtime = h.runtime();
    runtime.origin = Origin::Ac;
    runtime.permission = Some(Permission::new(move || {
        anyhow::ensure!(
            device.0.lock().unwrap().pages[6][33..37] != 77i32.to_le_bytes(),
            "供电条件已变化"
        );
        Ok(())
    }));
    let report = h
        .application
        .begin(runtime, &h.file, None, Some(&h.review), None, None)
        .unwrap();
    let report = h
        .application
        .keep(&h.f.hal, &h.f.gpu, h.f.devices(), report.id)
        .unwrap();
    assert!(!report.targets_applied && report.pending.is_none());
    assert!(
        report
            .steps
            .iter()
            .any(|s| !s.ok && s.step == "档案授权复核")
    );
    assert_eq!(h.f.device.0.lock().unwrap().pages[6][33..37], initial);
    h.restored();
}
