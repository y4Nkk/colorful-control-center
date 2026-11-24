use super::*;
use crate::profiles::ProfileFile;
use crate::profiles::application::{Controller, Runtime};
fn setup() -> Fixture {
    let f = Fixture::new();
    f.touchpad_io.enable();
    f
}
fn target(f: &Fixture, enabled: bool) -> crate::touchpad::Target {
    crate::touchpad::Target {
        devices: f.touchpad_io.devices.lock().unwrap().clone(),
        enabled,
        saved_enabled: true,
    }
}
#[test]
fn capture_and_strict_roundtrip_keep_system_and_preference_independent() {
    let f = setup();
    let settings = crate::profiles::capture::capture_current(&f.hal, &f.gpu, f.devices(), &crate::profiles::ProfileGroup::ALL).unwrap();
    let touchpad = settings.touchpad.as_ref().unwrap();
    assert!(touchpad.enabled && !touchpad.saved_enabled);
    assert!(
        serde_json::from_str::<ProfileSettings>(&serde_json::to_string(&settings).unwrap())
            .unwrap()
            == settings
    );
    assert!(f.touchpad_io.writes.lock().unwrap().is_empty());
    let invalid = ProfileSettings {
        touchpad: Some(crate::touchpad::Target {
            devices: vec![],
            ..touchpad.clone()
        }),
        ..Default::default()
    };
    assert!(crate::profiles::validate(&invalid).is_err());
    let only = ProfileSettings {
        touchpad: Some(touchpad.clone()),
        ..Default::default()
    };
    crate::profiles::validate(&only).unwrap();
}
#[test]
fn later_gpu_failure_restores_this_operation_only() {
    let f = setup();
    let id = f.touchpad.hotkey_identity(&f.hal).unwrap();
    assert!(!f.touchpad.toggle(&f.hal, &id).unwrap());
    let toggled = f.touchpad.read(&f.hal).unwrap().current;
    assert_eq!(toggled.enabled, Some(false));
    let settings = ProfileSettings {
        touchpad: Some(target(&f, true)),
        gpu: Some(crate::gpuctl::GpuClockSettings {
            core_offset_mhz: Some(60),
            mem_offset_mhz: None,
        }),
        ..Default::default()
    };
    f.device.0.lock().unwrap().fail_gpu = true;
    let steps = apply_settings(&f.hal, &f.gpu, f.devices(), &settings).unwrap();
    assert!(steps.iter().any(|s| s.step == "GPU 偏移" && !s.ok));
    assert!(steps.iter().any(|s| s.step == "恢复本次触摸板状态" && s.ok));
    assert_eq!(f.touchpad.read(&f.hal).unwrap().current, toggled);
}
#[test]
fn replaced_device_and_unknown_state_reject_all_targets_before_cpu_writes() {
    for unknown in [false, true] {
        let f = setup();
        let settings = ProfileSettings {
            touchpad: Some(target(&f, false)),
            pl1_w: Some(77.0),
            ..Default::default()
        };
        if unknown {
            *f.touchpad_io.enabled.lock().unwrap() = None;
        } else {
            *f.touchpad_io.devices.lock().unwrap() = vec!["replacement".into()];
        }
        assert!(apply_settings(&f.hal, &f.gpu, f.devices(), &settings).is_err());
        assert!(f.touchpad_io.writes.lock().unwrap().is_empty());
        assert!(f.device.0.lock().unwrap().events.is_empty());
    }
}
#[test]
fn every_local_failure_stops_and_restores_saved_cpu_and_shared_page_fields() {
    for step in [1, 2] {
        let f = setup();
        let before = f.touchpad.read(&f.hal).unwrap().current;
        let settings = ProfileSettings {
            touchpad: Some(target(&f, false)),
            pl1_w: Some(77.0),
            ..Default::default()
        };
        f.touchpad_io.fail_at.lock().unwrap().push(step);
        f.device.0.lock().unwrap().pages[1][41] = 77;
        let steps = apply_settings(&f.hal, &f.gpu, f.devices(), &settings).unwrap();
        assert!(steps.iter().any(|s| s.step == "触摸板开关" && !s.ok));
        assert!(!steps.iter().any(|s| s.step == "恢复本次触摸板状态"));
        assert_eq!(f.touchpad.read(&f.hal).unwrap().current, before);
        let state = f.device.0.lock().unwrap();
        assert_eq!(&state.pages[6][33..37], &65i32.to_le_bytes());
        assert_eq!(state.pages[1][41], 77);
    }
}
#[test]
fn local_recovery_failure_is_retried_by_outer_transaction() {
    let f = setup();
    let before = f.touchpad.read(&f.hal).unwrap().current;
    f.touchpad_io.fail_at.lock().unwrap().extend([2, 3]);
    let settings = ProfileSettings {
        touchpad: Some(target(&f, false)),
        ..Default::default()
    };
    let steps = apply_settings(&f.hal, &f.gpu, f.devices(), &settings).unwrap();
    assert!(steps.iter().any(|s| s.step == "触摸板开关" && !s.ok));
    assert!(steps.iter().any(|s| s.step == "恢复本次触摸板状态" && s.ok));
    assert_eq!(f.touchpad.read(&f.hal).unwrap().current, before);
}

#[test]
fn manual_and_startup_apply_same_file_with_independent_preference() {
    for startup in [false, true] {
        let f = setup();
        let dir = tempfile::tempdir().unwrap();
        let file = ProfileFile {
            version: 1,
            name: "触摸板".into(),
            saved_at_ms: 1,
            settings: ProfileSettings {
                touchpad: Some(target(&f, false)),
                ..Default::default()
            },
        };
        std::fs::write(
            dir.path().join("触摸板.json"),
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
        let actual = f.touchpad.read(&f.hal).unwrap().current;
        assert_eq!(actual.enabled, Some(false));
        assert_eq!(actual.saved, 1);
    }
}
