#[test]
fn recovery_132_is_independent_and_high_codes_keep_original_low_byte_decoding() {
    use super::{Action, Packet, decode};
    assert_eq!(
        Packet { code: 132, data: 0 }.action(),
        Some(Action::OpenRecovery)
    );
    assert!(!Action::OpenRecovery.is_status_feedback());
    assert_eq!(
        decode("Arg001^301").unwrap(),
        Some(Packet { code: 45, data: 1 })
    );
    assert_eq!(
        decode("Arg001^304").unwrap(),
        Some(Packet { code: 48, data: 1 })
    );
}
#[test]
fn battery_alert_uses_original_bit7_absolute_value_and_has_a_separate_stop() {
    for data in [0, 1, 127, 128, 170, 228, 255, -1] {
        let packet = super::decode(&format!("Arg001^{}", (data << 8) | 186))
            .unwrap()
            .unwrap();
        assert_eq!(
            packet.action(),
            Some(super::Action::BatteryAlert {
                active: true,
                brightness: (data & 0x80 != 0).then_some((data & 0x7f) as u8)
            })
        );
        assert_eq!(
            super::Packet { code: 187, data }.action(),
            Some(super::Action::BatteryAlert {
                active: false,
                brightness: None
            })
        );
    }
}
#[test]
fn keyboard_navigation_event_uses_original_type_whitelist() {
    for value in 0..=255 {
        assert_eq!(
            super::keyboard_navigation_type_allowed(value),
            [1, 2, 3, 6, 22, 23, 38].contains(&value)
        );
    }
    assert_eq!(
        super::Packet { code: 131, data: 0 }.action(),
        Some(super::Action::OpenKeyboard)
    );
}
#[test]
fn rotation_event_is_separate_and_ignores_the_original_data_byte() {
    for data in [-1, 0, 1, 127] {
        assert_eq!(
            super::Packet { code: 124, data }.action(),
            Some(super::Action::DisplayRotation)
        );
    }
}
#[test]
fn low_battery_system_events_are_separate_from_brightness_and_thresholds() {
    for code in [44, 45] {
        for data in [-1, 0, 1, 127] {
            assert_eq!(
                super::Packet { code, data }.action(),
                Some(super::Action::BatteryLowAction(code))
            );
        }
    }
}
#[test]
fn low_battery_brightness_events_keep_their_own_action() {
    for code in [59, 60] {
        for data in [-1, 0, 1, 127] {
            assert_eq!(
                super::Packet { code, data }.action(),
                Some(super::Action::BatteryLowBrightness(code))
            );
        }
    }
}
#[test]
fn original_battery_cold_event_is_read_only_feedback() {
    for data in [-1, 0, 1, 127] {
        let action = super::Packet { code: 47, data }.action().unwrap();
        assert_eq!(action, super::Action::BatteryColdWarning);
        assert!(action.is_status_feedback());
    }
}
#[test]
fn original_pd_event_is_independent_of_low_battery_actions() {
    for data in [-1, 0, 1, 127] {
        assert_eq!(
            super::Packet { code: 46, data }.action(),
            Some(super::Action::PowerDeliveryEvent)
        );
    }
}
#[test]
fn battery_threshold_events_have_independent_actions() {
    for code in [209, 210] {
        for data in [-1, 0, 1, 127] {
            assert_eq!(
                super::Packet { code, data }.action(),
                Some(super::Action::BatteryThreshold(code))
            );
        }
    }
}
#[test]
fn screen_key_215_keeps_its_own_action_regardless_of_payload() {
    for data in [-1, 0, 1, 127] {
        assert_eq!(
            super::Packet { code: 215, data }.action(),
            Some(super::Action::PanelHotkey)
        );
    }
}
#[test]
fn gpu_turbo_codes_have_their_own_actions_without_payload_aliases() {
    for code in [96, 97] {
        for data in [-1, 0, 1, 127] {
            assert_eq!(
                super::Packet { code, data }.action(),
                Some(super::Action::GpuTurbo(code))
            );
        }
    }
}
use super::*;

#[test]
fn airplane_and_panel_events_keep_independent_semantics() {
    assert_eq!(
        Packet { code: 133, data: 0 }.action(),
        Some(Action::ToggleAirplane)
    );
    assert_eq!(
        Packet { code: 84, data: 0 }.action(),
        Some(Action::PanelEvent)
    );
}
#[test]
fn camera_feedback_has_exact_report_values_and_cache_side_effects() {
    for (code, report, saved) in [
        (246, CameraReport::Camera(false), Some(0)),
        (247, CameraReport::Camera(true), Some(1)),
        (248, CameraReport::CameraMicrophone(false), None),
        (249, CameraReport::CameraMicrophone(true), Some(1)),
    ] {
        for data in [-1, 0, 1, 127] {
            let action = Packet { code, data }.action().unwrap();
            assert_eq!(action, Action::CameraReport(report));
            assert_eq!(report.saved_camera(), saved);
            assert!(action.is_status_feedback());
        }
    }
    assert!(!Action::ToggleMicrophones.is_status_feedback());
    assert!(!Action::KeyboardBrightnessEvent.is_status_feedback());
    assert!(Action::Notify(Notice::CapsLock(true)).is_status_feedback());
}
#[test]
fn exact_oem_status_codes_are_notifications_with_original_polarity() {
    for (code, notice) in [
        (166, Notice::Docking(true)),
        (167, Notice::Docking(false)),
        (184, Notice::Gps(false)),
        (185, Notice::Gps(true)),
        (199, Notice::NumLock(false)),
        (200, Notice::NumLock(true)),
        (201, Notice::CapsLock(false)),
        (202, Notice::CapsLock(true)),
        (203, Notice::ScrollLock(false)),
        (204, Notice::ScrollLock(true)),
        (222, Notice::SilentMode(true)),
        (223, Notice::SilentMode(false)),
        (234, Notice::MobileNetwork(false)),
        (235, Notice::MobileNetwork(true)),
        (64, Notice::DustRunning),
        (58, Notice::TurboFan),
        (240, Notice::Charger(true)),
        (241, Notice::Charger(false)),
    ] {
        assert_eq!(
            Packet { code, data: 0 }.action(),
            Some(Action::Notify(notice))
        );
        assert_eq!(
            Packet { code, data: 127 }.action(),
            Some(Action::Notify(notice))
        );
    }
    // 原OutLog入口先取低字节，301等switch分支并非可直接接收的Arg001事件。
    for (raw, expected) in [
        (300, Some(Action::BatteryLowAction(44))),
        (301, Some(Action::BatteryLowAction(45))),
        (302, Some(Action::PowerDeliveryEvent)),
        (303, Some(Action::BatteryColdWarning)),
        (304, Some(Action::DisplayBrightness(48))),
        (387, Some(Action::OpenKeyboard)),
        (380, Some(Action::DisplayRotation)),
    ] {
        let packet = decode(&format!("Arg001^{raw}")).unwrap().unwrap();
        assert_eq!(packet.code, raw as u8);
        assert_eq!(packet.data, raw >> 8);
        assert_eq!(packet.action(), expected);
    }
}
#[test]
fn signed_transport_matches_csharp_low_byte_and_arithmetic_shift() {
    assert_eq!(
        decode("Arg001^549").unwrap(),
        Some(Packet { code: 37, data: 2 })
    );
    assert_eq!(
        decode("Arg001^-2147483611").unwrap(),
        Some(Packet {
            code: 37,
            data: -8388608
        })
    );
    for message in [
        "Arg001^",
        "Arg001^+37",
        "Arg001^037",
        "Arg001^37^1",
        "Arg001^2147483648",
        "Arg001^ 37",
    ] {
        assert!(decode(message).is_err(), "{message}");
    }
    assert_eq!(decode("clevofnkey^20e").unwrap(), None);
}
#[test]
fn unsupported_amd_and_unknown_events_remain_unhandled() {
    for code in [65, 66, 67, 68, 69, 70, 71, 85, 90, 91, 93, 255] {
        assert_eq!(Packet { code, data: 0 }.action(), None);
    }
    for (code, action) in [
        (37, Action::ToggleFnLock),
        (38, Action::ToggleMicrophones),
        (252, Action::ToggleTouchpad),
        (253, Action::ToggleTouchpad),
        (139, Action::ToggleKeyboardZone(0)),
        (140, Action::ToggleKeyboardZone(1)),
        (141, Action::ToggleKeyboardZone(2)),
        (57, Action::CycleKeyboardColor),
        (123, Action::ToggleMappings),
        (143, Action::FanMode(1)),
        (112, Action::FanMode(0)),
        (129, Action::KeyboardBrightness(-1)),
    ] {
        assert_eq!(Packet { code, data: 0 }.action(), Some(action));
    }
}

#[test]
fn shutdown_and_lid_events_never_alias_low_battery_or_each_other() {
    for data in [-1, 0, 1, 127] {
        assert_eq!(
            Packet { code: 72, data }.action(),
            Some(Action::SystemShutdown)
        );
        assert_eq!(Packet { code: 192, data }.action(), Some(Action::LidAction));
    }
    assert!(!Action::SystemShutdown.is_status_feedback());
    assert!(!Action::LidAction.is_status_feedback());
}
#[test]
fn mode_cycle_skips_unsupported_slots_and_rejects_unknown_current_mode() {
    let available = [0, 1, 2, 3];
    for (before, after) in [(0, 2), (2, 3), (3, 1), (1, 0)] {
        assert_eq!(next_mode(before, &available).unwrap(), after);
    }
    assert_eq!(next_mode(1, &[1, 2, 3]).unwrap(), 2);
    assert!(next_mode(1, &[1, 2, 3, 4]).is_err());
    assert!(next_mode(4, &available).is_err());
    assert_eq!(next_mode(0, &[2]).unwrap(), 2);
    assert!(next_mode(0, &[]).is_err());
    assert!(next_mode(0, &[7]).is_err());
}
#[test]
fn only_mode_requests_have_three_second_cooldown_and_clock_rewind_is_rejected() {
    let mut gate = Gate::default();
    assert!(gate.accept(Action::CycleMode, Duration::from_secs(10)));
    assert!(!gate.accept(Action::CycleMode, Duration::from_millis(12999)));
    assert!(!gate.accept(Action::CycleMode, Duration::from_secs(9)));
    assert!(gate.accept(Action::KeyboardBrightness(1), Duration::from_secs(10)));
    assert!(gate.accept(Action::CycleMode, Duration::from_secs(13)));
}
#[test]
fn keyboard_switch_events_keep_white_rule_and_have_no_mode_cooldown() {
    let mut gate = Gate::default();
    for (code, rgb_only) in [(63, false), (158, true), (159, true)] {
        let action = Packet { code, data: 0 }.action().unwrap();
        assert_eq!(action, Action::ToggleKeyboard { rgb_only });
        assert!(gate.accept(action, Duration::from_secs(1)));
        assert!(gate.accept(action, Duration::from_secs(1)));
    }
    for (code, expected) in [
        (62, None),
        (64, Some(Action::Notify(Notice::DustRunning))),
        (138, Some(Action::KeyboardBrightnessEvent)),
        (142, None),
    ] {
        assert_eq!(Packet { code, data: 0 }.action(), expected);
    }
    for raw in [2, 3, 6, 22, 23, 38] {
        assert!(keyboard_toggle_type_allowed(true, raw));
    }
    for raw in [1, 19, 35, 51, 243, 254] {
        assert!(!keyboard_toggle_type_allowed(true, raw));
        assert!(keyboard_toggle_type_allowed(false, raw));
    }
}
