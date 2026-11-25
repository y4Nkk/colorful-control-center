use super::*;
use crate::fanctl::{self, FAN_MODE_AUTO, FAN_MODE_NOISELESS, OffsetSource, RestoreTarget};

fn source(mode: u8, slot: Option<usize>) -> OffsetSource {
    OffsetSource { mode, slot }
}

fn restore(hal: &Hal, target: RestoreTarget) -> anyhow::Result<()> {
    let expected = fanctl::read_restore_view(hal)?.restore_revision;
    fanctl::restore(hal, target, &expected)
}

#[test]
fn fan_offset_capability_respects_both_layout_and_wmi96() {
    let hal = hal();
    for (layout, flags, expected) in [(0, 0, true), (0, 4, false), (0x10, 0, false)] {
        wire(|w| {
            w.pages[7][11] = flags;
            w.pages[7][20] = layout;
        });
        let profile = crate::fanctl::read_firmware_profile(&hal).unwrap();
        assert_eq!(profile.offset_blocked_reason.is_none(), expected);
    }
    wire(|w| {
        w.pages[7][10] = 0;
        w.pages[7][11] = 0;
        w.pages[7][20] = 0;
    });
    assert!(
        crate::fanctl::read_firmware_profile(&hal)
            .unwrap()
            .offset_blocked_reason
            .unwrap()
            .contains("能力未知")
    );
    wire(|w| assert!(w.writes.is_empty()));
}

#[test]
fn noiseless_fan_offset_only_updates_the_offset_and_its_slot() {
    let hal = hal();
    let mut expected = wire(|w| {
        w.pages[7][20] = 1;
        w.pages[7][21] = 1;
        w.pages[1][1] = 3;
        w.pages[4][5] = FAN_MODE_NOISELESS;
        w.fan_mode_bits = 0x40;
        w.pages[4][7] = 20;
        w.pages[4][9] = 1;
        w.pages[4][73] = FAN_MODE_NOISELESS;
        w.pages[4][77] = 20;
        w.pages[4]
    });
    expected[7] = 50;
    expected[77] = 50;

    let _gate = hal.begin_write().unwrap();
    fanctl::set_manual_offset(&hal, source(FAN_MODE_NOISELESS, Some(3)), 50).unwrap();
    let profile = fanctl::read_profile(&hal).unwrap();
    assert_eq!(
        (profile.current_mode, profile.offset_pct),
        (FAN_MODE_NOISELESS, Some(50))
    );
    wire(|w| {
        assert_eq!(w.writes, [(121, vec![128, 0, 0, 14])]);
        assert_eq!(w.pages[4], expected);
    });
}

#[test]
fn manual_offset_rechecks_mode_capability_and_software_before_any_write() {
    use ccc_core::fan::{CurvePoint, FanCurve};

    let hal = hal();
    let _gate = hal.begin_write().unwrap();
    let expected = wire(|w| {
        w.pages[7][20] = 1;
        w.pages[4][5] = FAN_MODE_NOISELESS;
        w.fan_mode_bits = 0x40;
        w.pages[4]
    });
    assert!(
        fanctl::set_manual_offset(&hal, source(FAN_MODE_AUTO, None), 50)
            .unwrap_err()
            .to_string()
            .contains("档位已变化")
    );
    wire(|w| w.fan_mode_bits = 0x10);
    assert!(
        fanctl::set_manual_offset(&hal, source(FAN_MODE_AUTO, None), 50)
            .unwrap_err()
            .to_string()
            .contains("仅自动或寂静")
    );
    wire(|w| {
        w.fan_mode_bits = 0x40;
        w.pages[7][11] = 4;
    });
    assert!(
        fanctl::set_manual_offset(&hal, source(FAN_MODE_NOISELESS, None), 50)
            .unwrap_err()
            .to_string()
            .contains("机型不支持")
    );
    wire(|w| w.pages[7][11] = 0);
    let curve = FanCurve {
        points: vec![
            CurvePoint {
                temp_c: 50.0,
                duty_pct: 30.0,
            },
            CurvePoint {
                temp_c: 95.0,
                duty_pct: 100.0,
            },
        ],
        hysteresis_c: 2.0,
        ramp_pct_per_s: 20.0,
    };
    fanctl::software::inject_test_session(&hal, [("cpu".into(), curve)].into()).unwrap();
    assert!(
        fanctl::set_manual_offset(&hal, source(FAN_MODE_NOISELESS, None), 50)
            .unwrap_err()
            .to_string()
            .contains("软件接管")
    );
    assert!(fanctl::software::read(&hal).unwrap().is_some());
    assert!(hal.recovery().unwrap().fan.is_none());
    wire(|w| {
        assert!(w.writes.is_empty());
        assert_eq!(w.pages[4], expected);
    });
}

#[test]
fn entertainment_quiet_uses_the_ec_mode_and_rejects_stale_auto() {
    let hal = hal();
    let _gate = hal.begin_write().unwrap();
    wire(|w| {
        w.pages[7][20] = 1;
        w.pages[1][1] = 3;
        w.pages[4][7] = 20;
    });
    let auto = fanctl::read_profile(&hal).unwrap();
    assert_eq!((auto.current_mode, auto.offset_slot), (FAN_MODE_AUTO, None));

    wire(|w| w.pages[4][9] = 1);
    assert_eq!(
        fanctl::read_profile(&hal).unwrap().current_mode,
        FAN_MODE_AUTO
    );
    wire(|w| w.fan_mode_bits = 0x40);
    let quiet = fanctl::read_profile(&hal).unwrap();
    assert_eq!(quiet.current_mode, FAN_MODE_NOISELESS);
    assert!(quiet.offset_blocked_reason.is_none());
    assert_eq!(
        fanctl::checkpoint(&hal, false).unwrap().mode(),
        FAN_MODE_NOISELESS
    );
    let stale = source(auto.current_mode, auto.offset_slot);
    let error = fanctl::set_manual_offset(&hal, stale, 50).unwrap_err();
    assert!(error.to_string().contains("档位已变化"));
    wire(|w| assert!(w.writes.is_empty()));
    assert!(hal.recovery().unwrap().fan.is_none());

    fanctl::set_manual_offset(&hal, source(FAN_MODE_NOISELESS, None), 50).unwrap();
    wire(|w| {
        assert_eq!(w.writes, [(121, vec![128, 0, 0, 14])]);
        assert_eq!((w.pages[4][5], w.pages[4][7], w.pages[4][9]), (0, 50, 1));
    });
    assert_eq!(
        fanctl::read_profile(&hal).unwrap().current_mode,
        FAN_MODE_NOISELESS
    );
}

#[test]
fn stale_offset_slot_is_rejected_even_when_mode_and_offset_match() {
    let hal = hal();
    let _gate = hal.begin_write().unwrap();
    wire(|w| {
        w.pages[7][21] = 1;
        w.pages[1][1] = 2;
        w.pages[4][7] = 20;
        w.pages[4][76..78].copy_from_slice(&[20, 20]);
    });
    let performance = fanctl::read_profile(&hal).unwrap();
    assert_eq!(performance.offset_slot, Some(2));
    wire(|w| w.pages[1][1] = 3);
    let entertainment = fanctl::read_profile(&hal).unwrap();
    assert_eq!(entertainment.offset_slot, Some(3));
    assert_eq!(performance.current_mode, entertainment.current_mode);
    assert_eq!(performance.offset_pct, entertainment.offset_pct);
    let error = fanctl::set_manual_offset(&hal, source(FAN_MODE_AUTO, Some(2)), 50).unwrap_err();
    assert!(error.to_string().contains("记忆槽已变化"));
    assert!(hal.recovery().unwrap().fan.is_none());
    wire(|w| {
        assert!(w.writes.is_empty());
        assert_eq!(w.pages[4][7], 20);
        assert_eq!(&w.pages[4][76..78], &[20, 20]);
    });
    fanctl::set_manual_offset(&hal, source(FAN_MODE_AUTO, Some(3)), 50).unwrap();
    wire(|w| assert_eq!(&w.pages[4][76..78], &[20, 50]));
}

#[test]
fn partial_slot_save_failure_reissues_the_previous_offset_and_retains_first_baseline() {
    let hal = hal();
    let _gate = hal.begin_write().unwrap();
    wire(|w| {
        w.pages[7][20] = 1;
        w.pages[7][21] = 1;
        w.pages[1][1] = 2;
        w.pages[4][5] = FAN_MODE_NOISELESS;
        w.fan_mode_bits = 0x40;
        w.offset_raw = 77;
        w.pages[4][7] = 20;
        w.pages[4][76] = 20;
    });
    fanctl::set_manual_offset(&hal, source(FAN_MODE_NOISELESS, Some(2)), 40).unwrap();
    wire(|w| {
        w.writes.clear();
        w.offset_save_failure = Some(76);
    });
    let error =
        fanctl::set_manual_offset(&hal, source(FAN_MODE_NOISELESS, Some(2)), 50).unwrap_err();
    assert!(error.to_string().contains("已恢复"));
    wire(|w| {
        assert_eq!((w.pages[4][5], w.pages[4][7], w.pages[4][76]), (8, 40, 40));
        assert_eq!(
            w.writes,
            [(121, vec![128, 0, 0, 14]), (121, vec![102, 0, 0, 14])]
        );
        w.writes.clear();
    });
    restore(&hal, RestoreTarget::Initial).unwrap();
    wire(|w| {
        assert_eq!((w.pages[4][5], w.pages[4][7], w.pages[4][76]), (8, 20, 20));
        assert_eq!(w.writes, [(121, vec![77, 0, 0, 14])]);
    });
}

#[test]
fn offset_compensation_failure_is_reported_alongside_the_save_error() {
    let hal = hal();
    let _gate = hal.begin_write().unwrap();
    wire(|w| {
        w.pages[4][7] = 20;
        w.offset_raw = 77;
        w.offset_save_failure = Some(7);
        w.offset_command_failure = Some(77);
    });
    let error = fanctl::set_manual_offset(&hal, source(FAN_MODE_AUTO, None), 50).unwrap_err();
    let message = error.to_string();
    assert!(message.contains("offset=7"), "{message}");
    assert!(
        message.contains("即时偏移") && message.contains("恢复失败"),
        "{message}"
    );
    wire(|w| assert_eq!(w.pages[4][7], 20));
}

#[test]
fn combined_restore_applies_initial_before_factory_and_reports_the_failed_stage() {
    for fault in [None, Some(77), Some(0)] {
        let hal = hal();
        let _gate = hal.begin_write().unwrap();
        wire(|w| {
            w.offset_raw = 77;
            w.pages[4][7] = 30;
        });
        fanctl::set_manual_offset(&hal, source(FAN_MODE_AUTO, None), 50).unwrap();
        wire(|w| {
            w.writes.clear();
            w.offset_command_failure = fault;
        });
        let result = restore(&hal, RestoreTarget::InitialAndFirmwareDefault);
        wire(|w| {
            assert_eq!(w.writes[0], (121, vec![77, 0, 0, 14]));
            let factory = w
                .writes
                .iter()
                .position(|(command, payload)| *command == 121 && payload == &[1, 0, 0, 34]);
            match fault {
                None => {
                    result.unwrap();
                    assert_eq!(factory, Some(1));
                    assert_eq!(w.offset_raw, 0);
                }
                Some(77) => {
                    assert!(
                        result
                            .unwrap_err()
                            .to_string()
                            .contains("初始风扇设置恢复失败")
                    );
                    assert_eq!(factory, None);
                    assert_eq!(w.offset_raw, 128);
                }
                Some(0) => {
                    assert!(
                        result
                            .unwrap_err()
                            .to_string()
                            .contains("初始风扇设置已恢复，固件默认设置恢复失败")
                    );
                    assert_eq!(factory, Some(1));
                    assert_eq!(w.offset_raw, 77);
                }
                _ => unreachable!(),
            }
        });
        assert!(hal.recovery().unwrap().fan.is_some());
    }
}

#[test]
fn changed_initial_checkpoint_revokes_confirmation_before_writing() {
    let hal = hal();
    let _gate = hal.begin_write().unwrap();
    wire(|w| w.offset_raw = 77);
    fanctl::set_manual_offset(&hal, source(FAN_MODE_AUTO, None), 50).unwrap();
    let expected = fanctl::read_restore_view(&hal).unwrap().restore_revision;
    let replacement = fanctl::checkpoint(&hal, false).unwrap();
    hal.recovery().unwrap().fan = Some(replacement);
    wire(|w| w.writes.clear());
    let error =
        fanctl::restore(&hal, RestoreTarget::InitialAndFirmwareDefault, &expected).unwrap_err();
    assert!(error.to_string().contains("风扇恢复目标已变化"));
    wire(|w| {
        assert!(w.writes.is_empty());
        assert_eq!(w.offset_raw, 128);
    });
}

#[test]
fn entertainment_turbo_restores_auto_without_erasing_quiet_preference() {
    use std::sync::atomic::Ordering;

    let hal = hal();
    let _gate = hal.begin_write().unwrap();
    wire(|w| {
        w.pages[7][10] |= 4;
        w.pages[7][20] = 1;
        w.pages[1][1] = 3;
        w.pages[4][7..10].copy_from_slice(&[20, 1, 1]);
        w.offset_raw = 51;
    });
    hal.with_dchu(|d| {
        d.test_entertainment_options.store(true, Ordering::Relaxed);
        d.acknowledged_mode.store(0x43, Ordering::Relaxed);
        Ok(())
    })
    .unwrap();
    assert_eq!(
        fanctl::read_profile(&hal).unwrap().current_mode,
        FAN_MODE_AUTO
    );
    fanctl::set_manual_offset(&hal, source(FAN_MODE_AUTO, None), 50).unwrap();
    assert_eq!(
        hal.recovery().unwrap().fan.as_ref().map(|fan| fan.mode()),
        Some(FAN_MODE_AUTO)
    );
    wire(|w| w.writes.clear());
    restore(&hal, RestoreTarget::Initial).unwrap();
    wire(|w| {
        assert_eq!(w.writes, [(121, vec![51, 0, 0, 14])]);
        assert_eq!(&w.pages[4][7..10], &[20, 1, 1]);
    });
    assert!(hal.recovery().unwrap().fan.is_none());
}

#[test]
fn offset_source_requires_an_explicit_shared_or_numbered_slot() {
    let shared = serde_json::from_str::<OffsetSource>(r#"{"mode":0,"slot":null}"#).unwrap();
    assert_eq!((shared.mode, shared.slot), (FAN_MODE_AUTO, None));
    for obsolete in [r#"{"mode":0}"#, r#"{"expectedMode":0,"slot":null}"#] {
        assert!(serde_json::from_str::<OffsetSource>(obsolete).is_err());
    }
}
