use super::*;
use crate::fanctl::custom::{Curves, Store};
use ccc_core::fan::{OemCurvePoints, oem_page4_offsets};

fn cpu_points() -> OemCurvePoints {
    OemCurvePoints {
        t2: 65.0,
        d2: 40.0,
        t3: 85.0,
        d3: 80.0,
    }
}

#[test]
fn custom_curves_save_across_modes_and_apply_only_on_explicit_custom_selection() {
    let f = Fixture::new();
    let dir = tempfile::tempdir().unwrap();
    let store = Store(dir.path().join("custom.json"));
    let _write = f.hal.begin_write().unwrap();
    let original = f.device.0.lock().unwrap().fan_curves;
    let revision = store.read(&f.hal).unwrap().custom.unwrap().revision;
    f.device.0.lock().unwrap().fan_mode = 1;
    let changes = cpu_changes(65.0);
    store.save(&f.hal, &revision, changes.clone()).unwrap();
    {
        let state = f.device.0.lock().unwrap();
        assert_eq!(state.fan_mode, 1);
        assert_eq!(state.fan_curves, original);
        assert!(state.events.is_empty());
    }
    let store = Store(store.0);
    let pending = store.read(&f.hal).unwrap().custom.unwrap();
    assert!(pending.pending);
    assert_eq!(pending.saved, Some(changes));
    store.select_mode(&f.hal, 0).unwrap();
    assert_eq!(f.device.0.lock().unwrap().fan_curves, original);
    store.select_mode(&f.hal, 6).unwrap();
    {
        let state = f.device.0.lock().unwrap();
        assert_eq!(state.fan_mode, 6);
        assert_eq!(state.fan_curves[0][1], 65);
        assert_eq!(state.fan_curves[1], original[1]);
    }
    let confirmed = store.read(&f.hal).unwrap().custom.unwrap();
    assert!(!confirmed.pending);
    store
        .save(&f.hal, &confirmed.revision, cpu_changes(68.0))
        .unwrap();
    assert_eq!(f.device.0.lock().unwrap().fan_curves[0][1], 65);
    store.select_mode(&f.hal, 6).unwrap();
    store.select_mode(&f.hal, 6).unwrap();
    let state = f.device.0.lock().unwrap();
    assert_eq!(state.fan_curves[0][1], 68);
    assert_eq!(state.events.iter().filter(|e| *e == "DCHU:121").count(), 2);
    assert_eq!(state.events.iter().filter(|e| *e == "DCHU:14").count(), 2);
}

#[test]
fn stale_or_invalid_curve_save_leaves_local_config_and_firmware_intact() {
    let f = Fixture::new();
    let dir = tempfile::tempdir().unwrap();
    let store = Store(dir.path().join("custom.json"));
    let _write = f.hal.begin_write().unwrap();
    let revision = store.read(&f.hal).unwrap().custom.unwrap().revision;
    let saved = cpu_changes(65.0);
    store.save(&f.hal, &revision, saved.clone()).unwrap();
    assert!(
        store
            .save(&f.hal, &revision, cpu_changes(67.0))
            .unwrap_err()
            .to_string()
            .contains("曲线配置已变化")
    );
    let revision = store.read(&f.hal).unwrap().custom.unwrap().revision;
    assert!(store.save(&f.hal, &revision, cpu_changes(99.0)).is_err());
    f.device.0.lock().unwrap().fan_curves[0][1] += 1;
    let stale = store
        .save(&f.hal, &revision, cpu_changes(66.0))
        .unwrap_err();
    assert!(stale.to_string().contains("曲线配置已变化"));
    assert_eq!(
        store.read(&f.hal).unwrap().custom.unwrap().saved,
        Some(saved)
    );
    assert!(f.device.0.lock().unwrap().events.is_empty());
}

fn cpu_changes(t2: f64) -> Curves {
    let points = OemCurvePoints { t2, ..cpu_points() };
    Curves::TwoPoint {
        curves: [("cpu".into(), points.into())].into(),
    }
}

#[test]
fn failed_activation_keeps_saved_rpm_curves_available_for_retry() {
    let f = Fixture::new();
    let original = install_rpm_tables(&f);
    let dir = tempfile::tempdir().unwrap();
    let store = Store(dir.path().join("custom.json"));
    let _write = f.hal.begin_write().unwrap();
    let profile = fanctl::read_profile(&f.hal).unwrap();
    let mut points = profile.rpm_fans[0].points;
    points[0].rpm_pct = 20;
    let changes = Curves::RpmTable {
        curves: [("cpu".into(), points)].into(),
    };
    let revision = store.read(&f.hal).unwrap().custom.unwrap().revision;
    store.save(&f.hal, &revision, changes.clone()).unwrap();
    {
        let mut state = f.device.0.lock().unwrap();
        assert!(state.events.is_empty());
        assert_eq!(state.policy_tables[&40], original);
        state.fail_rpm_write = true;
    }
    assert!(store.select_mode(&f.hal, 6).is_err());
    {
        let state = f.device.0.lock().unwrap();
        assert_eq!(state.fan_mode, 0);
        assert_eq!(state.policy_tables[&40], original);
    }
    let pending = store.read(&f.hal).unwrap().custom.unwrap();
    assert_eq!(pending.saved, Some(changes));
    assert!(pending.pending);
    store.select_mode(&f.hal, 6).unwrap();
    let actual = fanctl::read_profile(&f.hal).unwrap();
    assert_eq!(actual.current_mode, 6);
    assert_eq!(actual.rpm_fans[0].points, points);
    assert_eq!(actual.rpm_fans[1].points, profile.rpm_fans[1].points);
    assert!(!store.read(&f.hal).unwrap().custom.unwrap().pending);
}

#[test]
fn cpu_curve_uses_verified_ack_and_leaves_gpu_live_and_saved_bytes_independent() {
    let f = Fixture::new();
    let (_, gpu_temperature, _) = oem_page4_offsets(1);
    let (gpu, old_cpu) = {
        let mut state = f.device.0.lock().unwrap();
        state.fan_mode = 1;
        state.offset_raw = 77;
        state.pages[4][7] = 10;
        state.pages[4][gpu_temperature + 1] = 65;
        (state.fan_curves[1], state.fan_curves[0])
    };
    let _write = f.hal.begin_write().unwrap();
    assert_eq!(fanctl::read_profile(&f.hal).unwrap().current_mode, 1);
    fanctl::apply_curve(&f.hal, Some(cpu_points()), None, None).unwrap();
    {
        let state = f.device.0.lock().unwrap();
        assert_eq!(state.fan_curves[0][1], 65);
        assert_eq!(state.fan_curves[1], gpu);
        assert_eq!(state.pages[4][gpu_temperature + 1], 65);
        assert_eq!(state.fan_mode, 6);
    }
    f.restore_fan(fanctl::RestoreTarget::Initial).unwrap();
    let state = f.device.0.lock().unwrap();
    assert_eq!(state.fan_curves[0], old_cpu);
    assert_eq!(state.fan_curves[1], gpu);
    assert_eq!(
        (
            state.fan_mode,
            state.offset_raw,
            state.pages[4][5],
            state.pages[4][7]
        ),
        (1, 77, 0, 10)
    );
}

#[test]
fn manual_auto_waits_for_ec_readback_before_saving() {
    let f = Fixture::new();
    {
        let mut state = f.device.0.lock().unwrap();
        state.pages[1][1] = 2;
        state.pages[4][5] = 8;
        state.fan_mode = 2;
        state.fan_readbacks_on_commands = [[0x08, 0x08, 0x02].into()].into();
    }
    let _write = f.hal.begin_write().unwrap();
    assert_eq!(fanctl::set_mode(&f.hal, 0).unwrap(), 0);
    let state = f.device.0.lock().unwrap();
    assert!(state.fan_readbacks.is_empty());
    assert_eq!((state.fan_mode, state.pages[4][5]), (0, 0));
    assert_eq!(state.events.iter().filter(|e| *e == "DCHU:121").count(), 1);
}

#[test]
fn mode_ack_without_a_matching_ec_readback_cannot_commit_saved_mode() {
    let f = Fixture::new();
    {
        let mut state = f.device.0.lock().unwrap();
        state.pages[1][1] = 2;
        state.pages[4][5] = 8;
        state.fan_mode = 2;
        state.ignored_fan_writes = 2;
    }
    let _write = f.hal.begin_write().unwrap();
    let message = fanctl::set_mode(&f.hal, 0).unwrap_err().to_string();
    assert!(
        message.contains("EC 风扇档位回读为 2，目标为 0"),
        "{message}"
    );
    assert!(
        message.contains("预期 0x02")
            && message.contains("cmd13[14]")
            && message.contains("ms=0x08"),
        "{message}"
    );
    assert!(message.contains("已恢复写入前状态"), "{message}");
    let state = f.device.0.lock().unwrap();
    assert_eq!((state.fan_mode, state.pages[4][5]), (2, 8));
    let commands: Vec<_> = state
        .events
        .iter()
        .filter_map(|e| e.strip_prefix("FAN_MODE:"))
        .collect();
    assert_eq!(commands, ["0", "0", "2"]);
    assert_eq!(state.fan_command, Some(2));
    assert!(
        !state
            .events
            .iter()
            .any(|event| event.starts_with("PAGE4:5:"))
    );
}

#[test]
fn transient_unknown_mode_readback_can_settle_to_the_exact_target() {
    let f = Fixture::new();
    {
        let mut state = f.device.0.lock().unwrap();
        state.pages[1][1] = 2;
        state.pages[4][5] = 8;
        state.fan_mode = 2;
        state.fan_readbacks_on_commands = [[0x80, 0x02].into()].into();
    }
    let _write = f.hal.begin_write().unwrap();
    assert_eq!(fanctl::set_mode(&f.hal, 0).unwrap(), 0);
    let state = f.device.0.lock().unwrap();
    assert_eq!((state.fan_mode, state.pages[4][5]), (0, 0));
    assert_eq!(
        state
            .events
            .iter()
            .filter(|event| event.starts_with("FAN_MODE:"))
            .count(),
        1
    );
    assert!(state.fan_readbacks.is_empty());
}

#[test]
fn firmware_default_restores_curve_and_offset_if_auto_never_appears() {
    let f = Fixture::new();
    let (curves, page) = {
        let mut state = f.device.0.lock().unwrap();
        state.pages[1][1] = 2;
        state.fan_mode = 2;
        state.fan_curves[0][1] = 65;
        state.pages[4][5] = 8;
        state.pages[4][7] = 40;
        state.offset_raw = 102;
        state.ignored_fan_writes = 2;
        (state.fan_curves, state.pages[4])
    };
    let _write = f.hal.begin_write().unwrap();
    let message = f
        .restore_fan(fanctl::RestoreTarget::FirmwareDefault)
        .unwrap_err()
        .to_string();
    assert!(
        message.contains("EC 风扇档位回读为 2，目标为 0"),
        "{message}"
    );
    assert!(message.contains("已恢复写入前状态"), "{message}");
    let state = f.device.0.lock().unwrap();
    assert!(state.events.iter().any(|event| event == "FAN_RESET"));
    assert_eq!(state.fan_curves, curves);
    assert_eq!(state.pages[4], page);
    assert_eq!((state.fan_mode, state.offset_raw), (2, 102));
}

#[test]
fn failed_curve_recovery_still_restores_saved_bytes_raw_offset_and_live_mode() {
    let f = Fixture::new();
    {
        let mut state = f.device.0.lock().unwrap();
        state.fan_mode = 1;
        state.offset_raw = 77;
        state.pages[4][7] = 20;
    }
    let _write = f.hal.begin_write().unwrap();
    let before = fanctl::checkpoint(&f.hal, true).unwrap();
    {
        let mut state = f.device.0.lock().unwrap();
        state.fan_mode = 0;
        state.offset_raw = 100;
        state.pages[4][7] = 40;
        state.fail_curve_commands = 1;
    }
    let error = fanctl::restore_checkpoint(&f.hal, &before).unwrap_err();
    assert!(format!("{error:#}").contains("两点曲线"));
    let state = f.device.0.lock().unwrap();
    assert_eq!(
        (state.fan_mode, state.offset_raw, state.pages[4][7]),
        (1, 77, 20)
    );
}

#[test]
fn turbo_blocks_direct_and_fan_only_profile_curve_activation_before_writes() {
    let f = Fixture::new();
    {
        let mut state = f.device.0.lock().unwrap();
        state.pages[1][1] = 2;
        state.pages[4][8] = 1;
    }
    {
        let _write = f.hal.begin_write().unwrap();
        for result in [
            fanctl::set_mode(&f.hal, 6).map(|_| ()),
            fanctl::apply_curve(&f.hal, Some(cpu_points()), None, None),
        ] {
            assert!(format!("{:#}", result.unwrap_err()).contains("Turbo"));
        }
    }
    let settings = ProfileSettings {
        fan_mode: Some(6),
        ..Default::default()
    };
    let review = review::preview_manual(&f.hal, &f.gpu, f.devices(), &settings).unwrap();
    assert!(review.blocked.unwrap().contains("Turbo"));
    assert!(f.device.0.lock().unwrap().events.is_empty());
}

#[test]
fn panel_preset_and_profile_activate_existing_rpm_stop_level_as_is() {
    let f = Fixture::new();
    let response = install_rpm_tables(&f);
    {
        let _write = f.hal.begin_write().unwrap();
        assert_eq!(fanctl::set_mode(&f.hal, 6).unwrap(), 6);
        assert_eq!(
            fanctl::read_profile(&f.hal).unwrap().rpm_fans[0].points[0].rpm_pct,
            0
        );
        fanctl::set_mode(&f.hal, 0).unwrap();
        f.device.0.lock().unwrap().pages[4][73] = 6;
        let preset = powerctl::mode::checkpoint(&f.hal, 3).unwrap();
        assert_eq!(preset.target.fan.mode(), Some(6));
        preset.execute(&f.hal).unwrap();
        assert_eq!(f.device.0.lock().unwrap().fan_mode, 6);
        fanctl::set_mode(&f.hal, 0).unwrap();
    }
    let settings = ProfileSettings {
        fan_mode: Some(6),
        ..Default::default()
    };
    assert!(
        apply_settings(&f.hal, &f.gpu, f.devices(), &settings)
            .unwrap()
            .iter()
            .any(|step| step.step == "风扇模式" && step.ok)
    );
    let state = f.device.0.lock().unwrap();
    assert_eq!(state.fan_mode, 6);
    assert_eq!(state.policy_tables[&40], response);
}

#[test]
fn failed_rpm_default_cannot_override_a_verified_full_speed_rollback() {
    let f = Fixture::new();
    let original = install_rpm_tables(&f);
    {
        let mut state = f.device.0.lock().unwrap();
        state.fan_mode = 1;
        state.fail_rpm_write = true;
    }
    let _write = f.hal.begin_write().unwrap();
    let error = f
        .restore_fan(fanctl::RestoreTarget::FirmwareDefault)
        .unwrap_err();
    assert!(format!("{error:#}").contains("已恢复写入前状态"));
    let state = f.device.0.lock().unwrap();
    assert_eq!((state.fan_mode, state.fan_command), (1, Some(1)));
    assert_eq!(state.policy_tables[&40], original);
}

fn install_rpm_tables(f: &Fixture) -> [u8; 256] {
    let mut table = [0; 112];
    table[10..12].copy_from_slice(&5000u16.to_be_bytes());
    let mut response = [0; 256];
    response[..112].copy_from_slice(&ccc_core::rpm::encode_default(&table, 0, 3).unwrap());
    response[128..240].copy_from_slice(&ccc_core::rpm::encode_default(&table, 1, 3).unwrap());
    let mut state = f.device.0.lock().unwrap();
    state.bios[20] = 0x10;
    state.policy_tables.insert(40, response);
    response
}
