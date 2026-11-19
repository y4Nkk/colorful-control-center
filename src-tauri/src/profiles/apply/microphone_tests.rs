use super::*;
use crate::{
    microphone,
    profiles::{
        ProfileFile,
        application::{Controller, Runtime},
    },
};

fn setup() -> Fixture {
    let f = Fixture::new();
    f.microphone_io.enable();
    f
}
#[test]
fn missing_explicit_test_transport_never_opens_native_audio() {
    let f = Fixture::new();
    let error = microphone::Controller::default()
        .read(&f.hal)
        .err()
        .unwrap();
    assert!(error.to_string().contains("禁止回退原生接口"));
}
fn target(f: &Fixture, muted: bool) -> microphone::Target {
    let scope = f.microphones.scope().unwrap();
    let _gate = f.hal.begin_write().unwrap();
    let mut target = scope.capture(&f.hal).unwrap().unwrap();
    for endpoint in &mut target.endpoints {
        endpoint.muted = muted;
    }
    target.saved_muted = muted;
    target
}
#[test]
fn capture_and_strict_roundtrip_keep_mixed_endpoints_without_writes() {
    let f = setup();
    let settings = crate::profiles::capture_current(&f.hal, &f.gpu, f.devices(), &crate::profiles::ProfileGroup::ALL).unwrap();
    let microphones = settings.microphones.as_ref().unwrap();
    assert!(!microphones.endpoints[0].muted && microphones.endpoints[1].muted);
    assert!(!microphones.saved_muted);
    assert!(
        serde_json::from_str::<ProfileSettings>(&serde_json::to_string(&settings).unwrap())
            .unwrap()
            == settings
    );
    assert!(f.microphone_io.writes.lock().unwrap().is_empty());
}
#[test]
fn single_hotkey_and_profile_share_first_mixed_baseline_and_later_gpu_rollback() {
    let f = setup();
    let before = f.microphones.read(&f.hal).unwrap();
    let identity = f.microphones.hotkey_identity(&f.hal).unwrap();
    assert!(f.microphones.toggle(&f.hal, &identity).unwrap());
    assert!(
        f.microphones
            .read(&f.hal)
            .unwrap()
            .current
            .endpoints
            .iter()
            .all(|e| e.muted)
    );
    let settings = ProfileSettings {
        microphones: Some(target(&f, false)),
        gpu: Some(crate::gpuctl::GpuClockSettings {
            core_offset_mhz: Some(60),
            mem_offset_mhz: None,
        }),
        ..Default::default()
    };
    f.device.0.lock().unwrap().fail_gpu = true;
    let steps = apply_settings(&f.hal, &f.gpu, f.devices(), &settings).unwrap();
    assert!(steps.iter().any(|s| s.step == "GPU 偏移" && !s.ok));
    assert!(steps.iter().any(|s| s.step == "恢复本次麦克风状态" && s.ok));
    let profile = f.microphones.read(&f.hal).unwrap();
    assert!(profile.current.endpoints.iter().all(|e| e.muted));
    assert_eq!(profile.initial.as_ref(), Some(&before.current));
    f.microphones.restore(&f.hal, &profile.revision).unwrap();
    let restored = f.microphones.read(&f.hal).unwrap();
    assert_eq!(restored.current, before.current);
    assert!(!restored.can_restore && restored.initial.is_none());
}
#[test]
fn changed_endpoint_set_rejects_all_domains_before_saved_cpu_writes() {
    let f = setup();
    let settings = ProfileSettings {
        microphones: Some(target(&f, true)),
        pl1_w: Some(77.0),
        ..Default::default()
    };
    f.microphone_io.endpoints.lock().unwrap()[1].id = "replacement".into();
    assert!(apply_settings(&f.hal, &f.gpu, f.devices(), &settings).is_err());
    assert!(f.device.0.lock().unwrap().events.is_empty());
    assert!(f.microphone_io.writes.lock().unwrap().is_empty());
}
#[test]
fn every_local_failure_stops_and_restores_cpu_without_duplicate_microphone_recovery() {
    for failed in [1, 2] {
        let f = setup();
        let before = f.microphones.read(&f.hal).unwrap().current;
        let settings = ProfileSettings {
            microphones: Some(target(&f, true)),
            pl1_w: Some(77.0),
            ..Default::default()
        };
        f.microphone_io.fail_at.lock().unwrap().push(failed);
        let steps = apply_settings(&f.hal, &f.gpu, f.devices(), &settings).unwrap();
        assert!(steps.iter().any(|s| s.step == "麦克风静音" && !s.ok));
        assert!(!steps.iter().any(|s| s.step == "恢复本次麦克风状态"));
        assert_eq!(f.microphones.read(&f.hal).unwrap().current, before);
        assert_eq!(
            &f.device.0.lock().unwrap().pages[6][33..37],
            &65i32.to_le_bytes()
        );
    }
}
#[test]
fn failed_local_compensation_is_retried_using_attempted_fields_and_keeps_user_restore() {
    let f = setup();
    let before = f.microphones.read(&f.hal).unwrap().current;
    let settings = ProfileSettings {
        microphones: Some(target(&f, true)),
        ..Default::default()
    };
    *f.microphone_io.fail_at.lock().unwrap() = vec![1, 2];
    let steps = apply_settings(&f.hal, &f.gpu, f.devices(), &settings).unwrap();
    assert!(steps.iter().any(|s| s.step == "恢复本次麦克风状态" && s.ok));
    assert_eq!(f.microphones.read(&f.hal).unwrap().current, before);
    assert!(
        f.microphone_io
            .writes
            .lock()
            .unwrap()
            .iter()
            .all(|write| write.starts_with("A:"))
    );
    assert!(f.microphones.read(&f.hal).unwrap().can_restore);
}
#[test]
fn manual_and_startup_apply_the_same_microphone_file_target() {
    for startup in [false, true] {
        let f = setup();
        let dir = tempfile::tempdir().unwrap();
        let file = ProfileFile {
            version: 1,
            name: "麦克风".into(),
            saved_at_ms: 1,
            settings: ProfileSettings {
                microphones: Some(target(&f, true)),
                ..Default::default()
            },
        };
        std::fs::write(
            dir.path().join("麦克风.json"),
            serde_json::to_vec(&file).unwrap(),
        )
        .unwrap();
        let report = Controller::default()
            .begin(
                Runtime {
                    origin: if startup {
                        crate::profiles::application::Origin::Startup
                    } else {
                        crate::profiles::application::Origin::Manual
                    },
                    permission: startup
                        .then(|| crate::profiles::application::Permission::new(|| Ok(()))),
                    hal: &f.hal,
                    gpu: &f.gpu,
                    devices: f.devices(),
                    dir: dir.path(),
                },
                &file,
                None,
                None,
                None,
                None,
            )
            .unwrap();
        assert!(
            report.targets_applied
                && report.origin
                    == if startup {
                        crate::profiles::application::Origin::Startup
                    } else {
                        crate::profiles::application::Origin::Manual
                    }
        );
        assert!(
            f.microphones
                .read(&f.hal)
                .unwrap()
                .current
                .endpoints
                .iter()
                .all(|e| e.muted)
        );
    }
}
#[test]
fn old_preview_unknown_preference_and_invalid_target_do_not_write() {
    let f = setup();
    let profile = f.microphones.read(&f.hal).unwrap();
    f.device.0.lock().unwrap().pages[1][41] = 77;
    assert!(
        f.microphones
            .apply(
                &f.hal,
                &microphone::Request {
                    expected_revision: profile.revision,
                    muted: true
                }
            )
            .is_err()
    );
    assert!(crate::profiles::capture_current(&f.hal, &f.gpu, f.devices(), &crate::profiles::ProfileGroup::ALL).is_err());
    let settings = ProfileSettings {
        microphones: Some(microphone::Target {
            endpoints: vec![],
            saved_muted: true,
        }),
        ..Default::default()
    };
    assert!(crate::profiles::validate(&settings).is_err());
    assert!(f.microphone_io.writes.lock().unwrap().is_empty());
}

#[test]
fn changed_device_set_with_first_baseline_is_not_advertised_for_hotkey_takeover() {
    let f = setup();
    let identity = f.microphones.hotkey_identity(&f.hal).unwrap();
    f.microphones.toggle(&f.hal, &identity).unwrap();
    f.microphone_io.endpoints.lock().unwrap()[1].id = "replacement".into();
    f.microphone_io.writes.lock().unwrap().clear();
    let profile = f.microphones.read(&f.hal).unwrap();
    assert!(profile.blocked_reason.is_some() && !profile.can_restore);
    assert!(f.microphones.hotkey_identity(&f.hal).is_err());
    assert!(f.microphone_io.writes.lock().unwrap().is_empty());
}

#[test]
fn initial_restore_keeps_later_devices_untouched_and_releases_the_old_scope() {
    let f = setup();
    let identity = f.microphones.hotkey_identity(&f.hal).unwrap();
    f.microphones.toggle(&f.hal, &identity).unwrap();
    f.microphone_io
        .endpoints
        .lock()
        .unwrap()
        .push(microphone::Endpoint {
            id: "C".into(),
            name: Some("新麦克风".into()),
            muted: true,
        });
    f.microphone_io.writes.lock().unwrap().clear();
    let profile = f.microphones.read(&f.hal).unwrap();
    assert!(profile.blocked_reason.is_some() && profile.can_restore);
    assert!(f.microphones.hotkey_identity(&f.hal).is_err());
    f.microphones.restore(&f.hal, &profile.revision).unwrap();
    let after = f.microphones.read(&f.hal).unwrap();
    assert!(
        !after.current.endpoints[0].muted
            && after.current.endpoints[1].muted
            && after.current.endpoints[2].muted
    );
    assert!(after.initial.is_none() && after.blocked_reason.is_none());
    assert!(
        f.microphone_io
            .writes
            .lock()
            .unwrap()
            .iter()
            .all(|write| !write.starts_with("C:"))
    );
    assert!(f.microphones.hotkey_identity(&f.hal).is_ok());
}
