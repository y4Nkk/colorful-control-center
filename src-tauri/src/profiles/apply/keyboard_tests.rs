use super::*;
use crate::{keyboard, lighting};

fn setup(f: &Fixture, kind: u8) {
    let mut state = f.device.0.lock().unwrap();
    state.keyboard_live = [2, 1, 2, 7, 9];
    state.keyboard_boot_supported = true;
    state.bios[0] = 1;
    state.bios[7] |= 16;
    state.bios[8] |= 4;
    state.pages[2][4] = kind;
    for (at, bytes) in [
        (32, vec![4]),
        (33, vec![17, 2, 3]),
        (36, vec![1, 0, 0, 30]),
        (41, vec![1, 6, 7]),
        (80, vec![1, 10, 20, 30]),
        (84, vec![1, 11, 22, 33]),
        (88, vec![0, 44, 55, 66]),
        (92, vec![1, 77, 88, 99]),
        (96, vec![1, 0, 0, 255]),
        (100, vec![1, 255, 0, 0]),
        (104, vec![0, 0, 0, 255]),
        (108, vec![21; 19]),
    ] {
        state.pages[2][at..at + bytes.len()].copy_from_slice(&bytes);
    }
}
fn keyboard_target() -> keyboard::Values {
    keyboard::Values {
        fn_locked: Some(true),
        win_locked: Some(true),
        fn_win_swap: Some(true),
    }
}
fn light(kind: u8, enabled: bool) -> lighting::Target {
    let settings = if kind == 1 {
        serde_json::json!({"family":"white","brightness":3})
    } else {
        serde_json::json!({"family":"zoneRgb","brightness":2,"effect":"zones","allColor":{"r":1,"g":2,"b":3},
            "zones": (0..3).map(|_| serde_json::json!({"enabled":true,"color":{"r":0,"g":255,"b":127}})).collect::<Vec<_>>()})
    };
    serde_json::from_value(
        serde_json::json!({"keyboardType":kind,"enabled":enabled,"settings":settings}),
    )
    .unwrap()
}
fn options(kind: u8) -> lighting::options::Target {
    let rgb = kind != 1;
    serde_json::from_value(serde_json::json!({"keyboardType":kind,"values":{
        "idleTimer":{"enabled":true,"seconds":65},"bootOverride":true,
        "logo": if rgb { Some(serde_json::json!({"enabled":false,"color":{"r":0,"g":255,"b":0}})) } else { None },
        "lightBar": if rgb { Some(serde_json::json!({"enabled":true,"color":{"r":0,"g":0,"b":255}})) } else { None }
    }})).unwrap()
}
fn targets(kind: u8) -> ProfileSettings {
    ProfileSettings {
        keyboard: Some(keyboard_target()),
        keyboard_lighting: Some(light(kind, true)),
        lighting_options: Some(options(kind)),
        ..settings()
    }
}

#[test]
fn capture_disk_and_json_keep_keyboard_flags_and_disabled_nonzero_light_level() {
    for kind in [1, 2, 6] {
        let f = Fixture::new();
        setup(&f, kind);
        f.device.0.lock().unwrap().pages[2][80] = 0;
        let captured = crate::profiles::capture_current(&f.hal, &f.gpu, f.devices(), &crate::profiles::ProfileGroup::ALL).unwrap();
        assert_eq!(captured.keyboard.as_ref().unwrap().fn_locked, Some(false));
        let target = captured.keyboard_lighting.as_ref().unwrap();
        assert!(!target.enabled);
        assert_eq!(target.keyboard_type, kind);
        let json = serde_json::to_value(target).unwrap();
        assert_eq!(json["settings"]["brightness"], 3);
        let dir = tempfile::tempdir().unwrap();
        crate::profiles::save_in(dir.path(), captured.clone(), "键盘灯", None).unwrap();
        let restored = crate::profiles::load_from(dir.path(), "键盘灯").unwrap();
        assert!(restored.settings == captured);
        assert!(f.device.0.lock().unwrap().events.is_empty());
    }
}

#[test]
fn strict_device_targets_reject_empty_flags_bad_family_and_inconsistent_keyboard_types() {
    let mut settings = targets(2);
    settings.keyboard = Some(keyboard::Values {
        fn_locked: None,
        win_locked: None,
        fn_win_swap: None,
    });
    assert!(crate::profiles::validate(&settings).is_err());
    settings.keyboard = None;
    settings.lighting_options.as_mut().unwrap().keyboard_type = 6;
    assert!(crate::profiles::validate(&settings).is_err());
    settings.lighting_options = None;
    settings.keyboard_lighting.as_mut().unwrap().keyboard_type = 1;
    assert!(crate::profiles::validate(&settings).is_err());
    let mut value = serde_json::to_value(targets(2)).unwrap();
    value["keyboardLighting"]["legacy"] = true.into();
    assert!(serde_json::from_value::<ProfileSettings>(value).is_err());
}

#[test]
fn preflight_of_all_keyboard_domains_rejects_capability_or_type_changes_before_cpu() {
    let f = Fixture::new();
    setup(&f, 2);
    let mut settings = targets(2);
    settings
        .lighting_options
        .as_mut()
        .unwrap()
        .values
        .boot_override = None;
    assert!(apply_settings(&f.hal, &f.gpu, f.devices(), &settings).is_err());
    settings = targets(6);
    assert!(apply_settings(&f.hal, &f.gpu, f.devices(), &settings).is_err());
    settings = targets(2);
    f.device.0.lock().unwrap().keyboard_live[2] = 0;
    assert!(apply_settings(&f.hal, &f.gpu, f.devices(), &settings).is_err());
    assert!(f.device.0.lock().unwrap().events.is_empty());
}

#[test]
fn combined_targets_allow_planned_shared_field_changes_and_use_current_peripheral_mask() {
    let f = Fixture::new();
    setup(&f, 6);
    let original = f.device.0.lock().unwrap().pages[2];
    let steps = apply_settings(&f.hal, &f.gpu, f.devices(), &targets(6)).unwrap();
    assert!(steps.iter().all(|s| s.ok));
    for name in ["键盘功能", "键盘灯效", "灯光策略与外围灯区"] {
        assert!(steps.iter().any(|s| s.step == name));
    }
    let state = f.device.0.lock().unwrap();
    assert_eq!(state.keyboard_live, [1, 2, 1, 7, 9]);
    assert_eq!(&state.pages[1][36..39], &[1, 2, 1]);
    assert_eq!(&state.pages[2][36..40], &[1, 0, 1, 5]);
    assert!(state.events.iter().any(|s| s == "LIGHT:103:f04600ff"));
    assert_eq!(
        state
            .events
            .iter()
            .filter(|s| s.starts_with("LIGHT:103:e0"))
            .last()
            .unwrap(),
        "LIGHT:103:e0079001"
    );
    for at in [33, 34, 41, 42, 43, 96, 97, 98, 99, 108, 126] {
        assert_eq!(state.pages[2][at], original[at]);
    }
}

#[test]
fn later_gpu_failure_restores_keyboard_lighting_options_and_cpu_in_dependency_order() {
    let f = Fixture::new();
    setup(&f, 2);
    let original = f.device.0.lock().unwrap().pages[2];
    f.device.0.lock().unwrap().fail_gpu = true;
    let mut settings = targets(2);
    settings.gpu = Some(crate::gpuctl::GpuClockSettings {
        core_offset_mhz: Some(60),
        mem_offset_mhz: None,
    });
    let steps = apply_settings(&f.hal, &f.gpu, f.devices(), &settings).unwrap();
    let options = steps
        .iter()
        .position(|s| s.step == "恢复灯光策略与外围灯区" && s.ok)
        .unwrap();
    let lighting = steps
        .iter()
        .position(|s| s.step == "恢复键盘灯效" && s.ok)
        .unwrap();
    let keyboard = steps
        .iter()
        .position(|s| s.step == "恢复键盘功能" && s.ok)
        .unwrap();
    assert!(lighting < options && options < keyboard);
    let state = f.device.0.lock().unwrap();
    assert_eq!(state.pages[2], original);
    assert_eq!(state.keyboard_live, [2, 1, 2, 7, 9]);
    assert_eq!(&state.pages[1][36..39], &[0, 0, 0]);
    assert_eq!(&state.voltage[..2], &[-51, -26]);
    assert_eq!(
        state
            .events
            .iter()
            .filter(|s| s.starts_with("LIGHT:103:e0"))
            .last()
            .unwrap(),
        "LIGHT:103:e0053001"
    );
}

#[test]
fn partial_keyboard_save_compensates_and_stops_lighting_without_duplicate_restore() {
    let f = Fixture::new();
    setup(&f, 2);
    f.device.0.lock().unwrap().fail_keyboard_save = true;
    let steps = apply_settings(&f.hal, &f.gpu, f.devices(), &targets(2)).unwrap();
    assert!(
        steps
            .iter()
            .any(|s| s.step == "键盘功能" && !s.ok && s.message.contains("已恢复"))
    );
    assert!(
        !steps
            .iter()
            .any(|s| s.step == "键盘灯效" || s.step == "恢复键盘功能")
    );
    assert_eq!(f.device.0.lock().unwrap().keyboard_live, [2, 1, 2, 7, 9]);
}

#[test]
fn failed_immediate_lighting_compensation_is_retried_by_outer_transaction() {
    for failures in [1, 2] {
        let f = Fixture::new();
        setup(&f, 2);
        let original = f.device.0.lock().unwrap().pages[2];
        f.device.0.lock().unwrap().fail_lighting_words = failures;
        let steps = apply_settings(&f.hal, &f.gpu, f.devices(), &targets(2)).unwrap();
        assert!(steps.iter().any(|s| s.step == "键盘灯效" && !s.ok));
        assert_eq!(
            steps.iter().any(|s| s.step == "恢复键盘灯效" && s.ok),
            failures == 2
        );
        if failures == 1 {
            assert!(
                steps
                    .iter()
                    .any(|s| s.step == "键盘灯效" && s.message.contains("已恢复本次写入前配置"))
            );
        }
        assert!(!steps.iter().any(|s| s.step == "灯光策略与外围灯区"));
        assert_eq!(f.device.0.lock().unwrap().pages[2], original);
    }
}

#[test]
fn partial_peripheral_save_recovers_itself_then_rolls_back_earlier_keyboard_domains() {
    let f = Fixture::new();
    setup(&f, 2);
    let original = f.device.0.lock().unwrap().pages[2];
    f.device.0.lock().unwrap().fail_page2_at = Some(100);
    let steps = apply_settings(&f.hal, &f.gpu, f.devices(), &targets(2)).unwrap();
    assert!(
        steps
            .iter()
            .any(|s| s.step == "灯光策略与外围灯区" && !s.ok && s.message.contains("已恢复"))
    );
    assert!(!steps.iter().any(|s| s.step == "恢复灯光策略与外围灯区"));
    assert!(steps.iter().any(|s| s.step == "恢复键盘灯效" && s.ok));
    assert_eq!(f.device.0.lock().unwrap().pages[2], original);
}

#[test]
fn lighting_and_option_restores_do_not_clobber_keyboard_flags() {
    let f = Fixture::new();
    setup(&f, 2);
    let original = f.device.0.lock().unwrap().pages[2];
    let target = light(2, true);
    assert!(f.lighting.apply(&f.hal, target.settings).ok);
    let mut settings = targets(2);
    settings.keyboard_lighting.as_mut().unwrap().enabled = false;
    assert!(
        apply_settings(&f.hal, &f.gpu, f.devices(), &settings)
            .unwrap()
            .iter()
            .all(|s| s.ok)
    );
    let profile = f.lighting_options.read(&f.hal).unwrap();
    f.lighting_options
        .restore(&f.hal, &profile.revision)
        .unwrap();
    assert!(f.lighting.restore(&f.hal).ok);
    let state = f.device.0.lock().unwrap();
    assert_eq!(state.pages[2], original);
    assert_eq!(state.keyboard_live, [1, 2, 1, 7, 9]);
}

#[test]
fn disabled_white_target_keeps_saved_nonzero_level_and_only_sends_white_command() {
    let f = Fixture::new();
    setup(&f, 1);
    let target = ProfileSettings {
        keyboard_lighting: Some(light(1, false)),
        ..Default::default()
    };
    assert!(
        apply_settings(&f.hal, &f.gpu, f.devices(), &target)
            .unwrap()
            .iter()
            .all(|s| s.ok)
    );
    let state = f.device.0.lock().unwrap();
    assert_eq!((state.pages[2][35], state.pages[2][80]), (3, 0));
    assert!(state.events.iter().any(|s| s == "LIGHT:39:00000000"));
    assert!(!state.events.iter().any(|s| s.starts_with("LIGHT:103:")));
}

#[test]
fn profile_and_keyboard_or_peripheral_operations_follow_one_lock_order() {
    let f = Fixture::new();
    setup(&f, 2);
    let start = std::sync::Barrier::new(3);
    std::thread::scope(|threads| {
        threads.spawn(|| {
            start.wait();
            for _ in 0..10 {
                assert!(
                    apply_settings(&f.hal, &f.gpu, f.devices(), &targets(2))
                        .unwrap()
                        .iter()
                        .all(|s| s.ok)
                );
            }
        });
        threads.spawn(|| {
            start.wait();
            for _ in 0..10 {
                let current = f.keyboard.read(&f.hal).unwrap();
                let result = f.keyboard.apply(
                    &f.hal,
                    &keyboard::Request {
                        expected_revision: current.revision,
                        values: keyboard_target(),
                    },
                );
                if let Err(error) = result {
                    assert!(error.to_string().contains("已变化"));
                }
            }
        });
        threads.spawn(|| {
            start.wait();
            for _ in 0..10 {
                let current = f.lighting_options.read(&f.hal).unwrap();
                let result = f.lighting_options.apply(
                    &f.hal,
                    &lighting::options::Request {
                        expected_revision: current.revision,
                        values: current.current,
                    },
                );
                if let Err(error) = result {
                    assert!(error.to_string().contains("已变化"));
                }
            }
        });
    });
    let state = f.device.0.lock().unwrap();
    assert_eq!(state.keyboard_live, [1, 2, 1, 7, 9]);
    assert_eq!(&state.pages[1][36..39], &[1, 2, 1]);
}
