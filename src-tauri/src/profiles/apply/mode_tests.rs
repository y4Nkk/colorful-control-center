use super::*;

#[test]
fn real_mode_reader_uses_live_capabilities_without_initializing_the_oem_cache() {
    let f = Fixture::new();
    assert!(
        f.device.0.lock().unwrap().pages[7]
            .iter()
            .all(|value| *value == 0)
    );
    let before = f.device.0.lock().unwrap().events.clone();
    let profile = crate::powerctl::mode::read(&f.hal).unwrap();
    assert_eq!(profile.available_modes, [0, 1, 2, 3]);
    assert_eq!(f.device.0.lock().unwrap().events, before);
    assert!(
        f.device.0.lock().unwrap().pages[7]
            .iter()
            .all(|value| *value == 0)
    );
}
use crate::hal::power_plan::SCHEMES;

#[test]
fn firmware_dust_start_after_mode_preview_blocks_execution_and_restore_before_mutation() {
    let f = Fixture::new();
    let plan = powerctl::mode::checkpoint(&f.hal, 2).unwrap();
    {
        let mut state = f.device.0.lock().unwrap();
        state.bios[0] = 1;
        state.bios[10] |= 128;
        state.dust_running = true;
    }
    assert!(
        powerctl::mode::read(&f.hal)
            .unwrap()
            .blocked_reason
            .unwrap()
            .contains("除尘")
    );
    let mut wanted = settings();
    wanted.oem_power_mode = Some(2);
    assert!(
        apply_settings(&f.hal, &f.gpu, f.devices(), &wanted)
            .err()
            .unwrap()
            .to_string()
            .contains("除尘")
    );
    let _gate = f.hal.begin_write().unwrap();
    assert!(
        plan.execute(&f.hal)
            .unwrap_err()
            .to_string()
            .contains("除尘")
    );
    assert!(
        plan.restore(&f.hal)
            .unwrap_err()
            .to_string()
            .contains("除尘")
    );
    assert!(f.device.0.lock().unwrap().events.is_empty());
}

#[test]
fn calibration_record_blocks_profile_preflight_and_prepared_mode_execution_or_restore() {
    use crate::battery::calibration::{
        Baseline,
        store::{FileStore, Record, Store},
    };
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("battery-calibration.json");
    let mut f = Fixture::new();
    f.hal.bind_calibration_record(path.clone());
    let plan = powerctl::mode::checkpoint(&f.hal, 2).unwrap();
    let baseline: Baseline = serde_json::from_value(serde_json::json!({
        "stopThreshold":80, "powerScheme":"381b4222-f694-41f0-9685-ff5bb260df2e",
        "charge":{"current":{"enabled":false,"start":95,"stop":100},"startValues":[95],"stopValues":[100]},
        "energy":null
    })).unwrap();
    let record = Record::prepared("1234567890abcdef1234567890abcdef".into(), baseline).unwrap();
    FileStore(path.clone()).save(&record).unwrap();
    let mut wanted = settings();
    wanted.oem_power_mode = Some(2);
    assert!(
        apply_settings(&f.hal, &f.gpu, f.devices(), &wanted)
            .err()
            .unwrap()
            .to_string()
            .contains("校准")
    );
    assert!(
        powerctl::mode::read(&f.hal)
            .unwrap()
            .blocked_reason
            .is_some()
    );
    let _write = f.hal.begin_write().unwrap();
    assert!(
        plan.execute(&f.hal)
            .unwrap_err()
            .to_string()
            .contains("校准")
    );
    assert!(
        plan.restore(&f.hal)
            .unwrap_err()
            .to_string()
            .contains("校准")
    );
    assert!(f.device.0.lock().unwrap().events.is_empty());
    std::fs::write(&path, b"{broken").unwrap();
    assert!(powerctl::mode::checkpoint(&f.hal, 2).is_err());
    assert!(f.device.0.lock().unwrap().events.is_empty());
    std::fs::remove_file(&path).unwrap();
    plan.execute(&f.hal).unwrap();
    assert!(
        f.device
            .0
            .lock()
            .unwrap()
            .events
            .iter()
            .any(|event| event == "MODE:2")
    );
}

pub(super) fn enable_policy(f: &Fixture) {
    let mut state = f.device.0.lock().unwrap();
    state.bios[0] = 1;
    state.bios[17] = 0x8f;
    state.bios[16] = 8;
    state.bios[19] = 2;
    let mut aspm = [0; 256];
    aspm[..9].copy_from_slice(&[15, 0, 1, 1, 2, 2, 0, 0, 2]);
    let mut plan = [0; 256];
    plan[..5].copy_from_slice(&[29, 1, 0, 2, 0]);
    state.policy_tables.insert(15, aspm);
    state.policy_tables.insert(29, plan);
}
#[test]
fn mode_only_transaction_applies_quiet_plus_and_restores_fan_and_system_baseline() {
    let f = Fixture::new();
    enable_policy(&f);
    let _write = f.hal.begin_write().unwrap();
    let plan = powerctl::mode::checkpoint(&f.hal, 4).unwrap();
    plan.execute(&f.hal).unwrap();
    {
        let state = f.device.0.lock().unwrap();
        assert_eq!(state.pages[1][1], 4);
        assert_eq!(state.active_plan, SCHEMES[2]);
        assert_eq!(state.plan_values[&SCHEMES[0]], [1, 2]);
        assert!(state.events.iter().any(|e| e == "MODE:0"));
        assert_eq!(state.pages[4][5], 0);
        assert_eq!(state.pages[4][7], 0);
    }
    powerctl::mode::restore_checkpoint(&f.hal, &plan).unwrap();
    let state = f.device.0.lock().unwrap();
    assert_eq!(state.pages[1][1], 3);
    assert_eq!(state.active_plan, SCHEMES[0]);
    assert_eq!(state.plan_values[&SCHEMES[0]], [0, 1]);
}
#[test]
fn stale_linked_preview_is_rejected_before_any_mode_or_system_write() {
    let f = Fixture::new();
    enable_policy(&f);
    let expected = powerctl::mode::read(&f.hal)
        .unwrap()
        .targets
        .into_iter()
        .find(|t| t.mode == 4)
        .unwrap();
    f.device
        .0
        .lock()
        .unwrap()
        .policy_tables
        .get_mut(&15)
        .unwrap()[3] = 2;
    let _write = f.hal.begin_write().unwrap();
    let error = powerctl::mode::apply_preview(&f.hal, &expected)
        .unwrap_err()
        .to_string();
    assert!(error.contains("联动已变化"));
    assert!(f.device.0.lock().unwrap().events.is_empty());
}

#[test]
fn failed_plan_activation_restores_all_previous_targets_and_keeps_retryable_baseline() {
    let f = Fixture::new();
    enable_policy(&f);
    f.device.0.lock().unwrap().fail_plan = true;
    let _write = f.hal.begin_write().unwrap();
    assert!(
        powerctl::mode::checkpoint(&f.hal, 4)
            .unwrap()
            .execute(&f.hal)
            .is_err()
    );
    {
        let state = f.device.0.lock().unwrap();
        assert_eq!(state.pages[1][1], 3);
        assert_eq!(state.active_plan, SCHEMES[0]);
        assert_eq!(state.plan_values[&SCHEMES[0]], [0, 1]);
    }
    assert!(f.hal.recovery().unwrap().oem_mode.is_some());
}
#[test]
fn later_profile_gpu_failure_recovers_linked_policy_then_exact_runtime_cpu() {
    let f = Fixture::new();
    enable_policy(&f);
    let original_cpu = f.device.0.lock().unwrap().power;
    f.device.0.lock().unwrap().fail_gpu = true;
    let mut wanted = settings();
    wanted.oem_power_mode = Some(4);
    wanted.gpu = Some(crate::gpuctl::GpuClockSettings {
        core_offset_mhz: Some(60),
        mem_offset_mhz: None,
    });
    let steps = apply_settings(&f.hal, &f.gpu, f.devices(), &wanted).unwrap();
    assert!(
        steps
            .iter()
            .any(|step| step.step == "OEM 电源模式与联动" && step.ok)
    );
    assert!(steps.iter().any(|step| step.step == "GPU 偏移" && !step.ok));
    assert!(
        steps
            .iter()
            .filter(|step| step.step.starts_with("恢复"))
            .all(|step| step.ok)
    );
    let state = f.device.0.lock().unwrap();
    assert_eq!(state.power, original_cpu);
    assert_eq!(state.pages[1][1], 3);
    assert_eq!(state.active_plan, SCHEMES[0]);
    assert_eq!(state.plan_values[&SCHEMES[0]], [0, 1]);
    let mode = state
        .events
        .iter()
        .rposition(|event| event == "MODE:3")
        .unwrap();
    let cpu = state
        .events
        .iter()
        .rposition(|event| event == "UV:0:-51")
        .unwrap();
    assert!(mode < cpu);
}

fn custom_saved_fan(f: &Fixture) {
    let mut state = f.device.0.lock().unwrap();
    state.bios[0] = 1;
    state.bios[20] = 0x40;
    state.pages[4][73] = 6;
    state.pages[4][16..20].copy_from_slice(&[20, 45, 75, 100]);
    state.pages[4][22..26].copy_from_slice(&[40, 60, 80, 100]);
}
#[test]
fn default_fan_reset_synchronizes_defaults_and_is_fully_reversible() {
    let f = Fixture::new();
    custom_saved_fan(&f);
    {
        let mut state = f.device.0.lock().unwrap();
        state.offset_raw = 77;
        state.pages[4][7] = 20;
        state.pages[4][77] = 15;
    }
    let original = f.device.0.lock().unwrap().pages[4];
    let _write = f.hal.begin_write().unwrap();
    fanctl::restore_default(&f.hal).unwrap();
    {
        let state = f.device.0.lock().unwrap();
        assert_eq!(state.pages[4][73], 0);
        assert_eq!(
            (state.offset_raw, state.pages[4][7], state.pages[4][77]),
            (0, 0, 0)
        );
        assert_eq!(&state.pages[4][16..20], &[20, 40, 80, 100]);
    }
    fanctl::restore_initial(&f.hal).unwrap();
    let state = f.device.0.lock().unwrap();
    assert_eq!(state.pages[4], original);
    assert_eq!(state.offset_raw, 77);
}
#[test]
fn default_command_failure_retains_original_fan_state_and_recovery_path() {
    let f = Fixture::new();
    custom_saved_fan(&f);
    let original = f.device.0.lock().unwrap().pages[4];
    {
        let mut state = f.device.0.lock().unwrap();
        state.fan_mode = 1;
        state.fail_default = true;
    }
    let _write = f.hal.begin_write().unwrap();
    assert!(fanctl::restore_default(&f.hal).is_err());
    {
        let state = f.device.0.lock().unwrap();
        assert_eq!(state.pages[4], original);
        assert_eq!(state.fan_mode, 1);
        assert_eq!(state.fan_command, Some(1));
    }
    assert!(f.hal.recovery().unwrap().fan.is_some());
}

#[test]
fn partial_mode_save_failure_is_compensated_by_the_real_workflow() {
    let f = Fixture::new();
    f.device.0.lock().unwrap().pages[1][16] = 1;
    f.device.0.lock().unwrap().fail_mode_save = true;
    let expected = powerctl::mode::read(&f.hal)
        .unwrap()
        .targets
        .into_iter()
        .find(|target| target.mode == 2)
        .unwrap();
    let _write = f.hal.begin_write().unwrap();
    let error = powerctl::mode::apply_preview(&f.hal, &expected).unwrap_err();
    assert!(format!("{error:#}").contains("已恢复写入前状态"));
    let state = f.device.0.lock().unwrap();
    assert_eq!(state.pages[1][1], 3);
    assert_eq!(state.pages[1][16], 1);
    assert!(state.events.iter().any(|event| event == "MODE:2"));
    assert!(state.events.iter().any(|event| event == "MODE:3"));
}
#[test]
fn manual_profile_binds_linked_target_and_rejects_missing_or_stale_review() {
    let f = Fixture::new();
    enable_policy(&f);
    f.device.0.lock().unwrap().pages[1][16] = 2;
    let mut wanted = settings();
    wanted.oem_power_mode = Some(4);
    let file = crate::profiles::ProfileFile {
        version: 1,
        name: "联动".into(),
        saved_at_ms: 10,
        settings: wanted,
    };
    assert!(apply_reviewed(&f.hal, &f.gpu, f.devices(), &file, &file, None, None, None).is_err());
    assert!(f.device.0.lock().unwrap().events.is_empty());
    let old = crate::profiles::preview_mode(
        &f.devices().mode_context(&f.hal, &f.gpu).unwrap(),
        &file.settings,
    )
    .unwrap()
    .unwrap();
    f.device
        .0
        .lock()
        .unwrap()
        .policy_tables
        .get_mut(&15)
        .unwrap()[3] = 2;
    assert!(
        apply_reviewed(
            &f.hal,
            &f.gpu,
            f.devices(),
            &file,
            &file,
            Some(&old),
            None,
            None
        )
        .is_err()
    );
    assert!(f.device.0.lock().unwrap().events.is_empty());
    let fresh = crate::profiles::preview_mode(
        &f.devices().mode_context(&f.hal, &f.gpu).unwrap(),
        &file.settings,
    )
    .unwrap()
    .unwrap();
    assert_eq!(fresh.ac_mode, Some(4));
    assert!(
        apply_reviewed(
            &f.hal,
            &f.gpu,
            f.devices(),
            &file,
            &file,
            Some(&fresh),
            None,
            None
        )
        .unwrap()
        .iter()
        .all(|step| step.ok)
    );
    assert_eq!(f.device.0.lock().unwrap().pages[1][1], 4);
    assert_eq!(f.device.0.lock().unwrap().pages[1][16], 4);
    let stored = f.hal.recovery().unwrap().oem_mode.clone().unwrap().stored();
    let preview = stored.recovery_preview(&stored).unwrap();
    assert_eq!(preview.initial_ac_mode, Some(2));
}

#[path = "mode_plan_recovery_tests.rs"]
mod plan_recovery;
