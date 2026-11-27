use super::{Fixture, fanctl};
use ccc_core::fan::OemCurvePoints;

fn silent_fan() -> Fixture {
    let f = Fixture::new();
    {
        let mut state = f.device.0.lock().unwrap();
        state.pages[1][1] = 2;
        state.pages[4][5] = 8;
        state.fan_mode = 2;
    }
    f
}

fn mode_writes(events: &[String]) -> Vec<&str> {
    events
        .iter()
        .filter_map(|e| e.strip_prefix("FAN_MODE:"))
        .collect()
}

#[test]
fn an_acknowledged_but_dropped_auto_command_is_delivered_once_more() {
    let f = silent_fan();
    f.device.0.lock().unwrap().ignored_fan_writes = 1;
    let _write = f.hal.begin_write().unwrap();
    assert_eq!(fanctl::set_mode(&f.hal, 0).unwrap(), 0);
    let state = f.device.0.lock().unwrap();
    assert_eq!((state.fan_mode, state.pages[4][5]), (0, 0));
    assert_eq!(mode_writes(&state.events), ["0", "0"]);
    let last_command = state
        .events
        .iter()
        .rposition(|v| v == "FAN_MODE:0")
        .unwrap();
    let save = state
        .events
        .iter()
        .position(|v| v == "PAGE4:5:[0]")
        .unwrap();
    assert!(save > last_command);
}

#[test]
fn stable_silent_readback_from_a_different_initial_mode_can_reissue_auto() {
    let f = silent_fan();
    {
        let mut state = f.device.0.lock().unwrap();
        state.fan_mode = 1;
        state.ignored_fan_writes = 1;
        state.fan_readbacks_on_commands = [[0x08; 8].into()].into();
    }
    let _write = f.hal.begin_write().unwrap();
    assert_eq!(fanctl::set_mode(&f.hal, 0).unwrap(), 0);
    let state = f.device.0.lock().unwrap();
    assert_eq!(mode_writes(&state.events), ["0", "0"]);
    assert_eq!((state.fan_mode, state.pages[4][5]), (0, 0));
}

#[test]
fn an_isolated_auto_read_cannot_commit_a_dropped_manual_command() {
    let f = silent_fan();
    {
        let mut state = f.device.0.lock().unwrap();
        state.ignored_fan_writes = 1;
        state.fan_readbacks_on_commands = [[0x02].into()].into();
    }
    let _write = f.hal.begin_write().unwrap();
    let error = fanctl::set_mode(&f.hal, 0).unwrap_err().to_string();
    assert!(error.contains("写入前 EC 风扇档位 2"), "{error}");
    assert!(error.contains("已恢复写入前状态"), "{error}");
    let state = f.device.0.lock().unwrap();
    assert_eq!(mode_writes(&state.events), ["0", "2"]);
    assert!(!state.events.iter().any(|v| v == "PAGE4:5:[0]"));
    assert_eq!((state.fan_mode, state.pages[4][5]), (2, 8));
}

#[test]
fn changing_readbacks_during_confirmation_prevent_redelivery() {
    let f = silent_fan();
    {
        let mut state = f.device.0.lock().unwrap();
        state.ignored_fan_writes = 1;
        state.fan_readbacks_on_commands = [[0x10, 0x08].into()].into();
    }
    let _write = f.hal.begin_write().unwrap();
    let error = fanctl::set_mode(&f.hal, 0).unwrap_err().to_string();
    assert!(error.contains("已恢复写入前状态"), "{error}");
    assert!(error.contains("ms=0x10"), "{error}");
    let state = f.device.0.lock().unwrap();
    assert_eq!(mode_writes(&state.events), ["0", "2"]);
    assert_eq!((state.fan_mode, state.pages[4][5]), (2, 8));
}

#[test]
fn a_stale_original_read_cannot_skip_or_prematurely_confirm_compensation() {
    let f = silent_fan();
    f.device.0.lock().unwrap().fan_readbacks_on_commands =
        [[0x80; 8].into(), [0x08, 0x08, 0x02].into()].into();
    let _write = f.hal.begin_write().unwrap();
    let error = fanctl::set_mode(&f.hal, 0).unwrap_err().to_string();
    assert!(error.contains("已恢复写入前状态"), "{error}");
    let state = f.device.0.lock().unwrap();
    assert_eq!(mode_writes(&state.events), ["0", "2"]);
    assert!(state.fan_readbacks.is_empty());
    assert_eq!((state.fan_mode, state.pages[4][5]), (2, 8));
}

#[test]
fn rollback_can_reissue_the_original_mode_after_stable_auto_readback() {
    let f = silent_fan();
    f.device.0.lock().unwrap().fan_readbacks_on_commands =
        [[0x80; 8].into(), [0x02; 8].into()].into();
    let _write = f.hal.begin_write().unwrap();
    let error = fanctl::set_mode(&f.hal, 0).unwrap_err().to_string();
    assert!(error.contains("已恢复写入前状态"), "{error}");
    let state = f.device.0.lock().unwrap();
    assert_eq!(mode_writes(&state.events), ["0", "2", "2"]);
    assert_eq!((state.fan_mode, state.pages[4][5]), (2, 8));
}

#[test]
fn unconfirmed_compensation_reports_both_errors_and_can_be_retried() {
    let f = silent_fan();
    f.device.0.lock().unwrap().fan_readbacks_on_commands =
        [[0x80; 8].into(), [0xff; 8].into()].into();
    let _write = f.hal.begin_write().unwrap();
    let error = fanctl::set_mode(&f.hal, 0).unwrap_err().to_string();
    for detail in ["0x80", "恢复失败", "0xff"] {
        assert!(error.contains(detail), "{error}");
    }
    assert!(!error.contains("已恢复写入前状态"), "{error}");
    f.device.0.lock().unwrap().fan_readbacks.clear();
    f.restore_fan(fanctl::RestoreTarget::Initial).unwrap();
    let state = f.device.0.lock().unwrap();
    assert_eq!(mode_writes(&state.events), ["0", "2", "2"]);
    assert_eq!((state.fan_mode, state.pages[4][5]), (2, 8));
}

#[test]
fn a_transport_failure_never_redelivers_the_requested_mode() {
    let f = silent_fan();
    f.device.0.lock().unwrap().fail_fan_mode = true;
    let _write = f.hal.begin_write().unwrap();
    let error = fanctl::set_mode(&f.hal, 0).unwrap_err().to_string();
    assert!(error.contains("SetDCHU(cmd=121, sub=1)"), "{error}");
    let state = f.device.0.lock().unwrap();
    assert_eq!(mode_writes(&state.events), ["0", "2"]);
    assert_eq!((state.fan_mode, state.pages[4][5]), (2, 8));
}

#[test]
fn firmware_defaults_commit_only_after_the_reissued_auto_is_confirmed() {
    let f = silent_fan();
    let defaults = {
        let mut state = f.device.0.lock().unwrap();
        let defaults = state.fan_curves;
        state.fan_curves[0][1] = 65;
        state.pages[4][7] = 40;
        state.offset_raw = 102;
        state.ignored_fan_writes = 1;
        defaults
    };
    let _write = f.hal.begin_write().unwrap();
    f.restore_fan(fanctl::RestoreTarget::FirmwareDefault)
        .unwrap();
    let state = f.device.0.lock().unwrap();
    assert_eq!(mode_writes(&state.events), ["0", "0"]);
    assert_eq!((state.fan_mode, state.pages[4][5]), (0, 0));
    assert_eq!((state.offset_raw, state.pages[4][7]), (0, 0));
    assert_eq!(state.fan_curves, defaults);
}

#[test]
fn invalid_modes_cannot_start_a_write_or_leave_a_pending_recovery() {
    let f = silent_fan();
    let _write = f.hal.begin_write().unwrap();
    f.hal
        .with_dchu(|io| {
            assert!(io.set_fan_mode_verified(3).is_err());
            assert!(io.force_restore_fan_mode(4).is_err());
            Ok(())
        })
        .unwrap();
    f.hal.with_dchu(|io| io.restore_fan_mode(2)).unwrap();
    assert!(f.device.0.lock().unwrap().events.is_empty());
}

#[test]
fn a_rejected_custom_activation_writes_no_saved_curve_or_mode() {
    let f = silent_fan();
    let before_curves = {
        let mut state = f.device.0.lock().unwrap();
        state.ignored_fan_writes = 2;
        state.fan_curves
    };
    let _write = f.hal.begin_write().unwrap();
    let points = OemCurvePoints {
        t2: 65.0,
        d2: 40.0,
        t3: 85.0,
        d3: 80.0,
    };
    let error = fanctl::apply_curve(&f.hal, Some(points), None, None)
        .unwrap_err()
        .to_string();
    assert!(error.contains("已恢复写入前状态"), "{error}");
    let state = f.device.0.lock().unwrap();
    assert!(state.events.iter().any(|v| v == "DCHU:14"));
    assert!(!state.events.iter().any(|v| v.starts_with("PAGE4:")));
    assert_eq!(state.fan_curves, before_curves);
    assert_eq!((state.fan_mode, state.pages[4][5]), (2, 8));
}
