use super::*;
#[path = "battery_tests.rs"]
mod battery_tests;
#[path = "cover_tests.rs"]
mod cover_tests;
#[path = "display_tests.rs"]
mod display_tests;
#[path = "dust_tests.rs"]
mod dust_tests;
#[path = "energy_tests.rs"]
mod energy_tests;
#[path = "fan_mode_confirmation_tests.rs"]
mod fan_mode_confirmation_tests;
#[path = "fan_transaction_tests.rs"]
mod fan_transaction_tests;
#[path = "fixtures.rs"]
pub(in crate::profiles) mod fixtures;
#[path = "gpu_fixture.rs"]
mod gpu_fixture;
#[path = "gpu_tests.rs"]
mod gpu_tests;
#[path = "keyboard_switch_tests.rs"]
mod keyboard_switch_tests;
#[path = "keyboard_tests.rs"]
mod keyboard_tests;
#[path = "keymap_tests.rs"]
mod keymap_tests;
#[path = "lan_tests.rs"]
mod lan_tests;
#[path = "lightbar_tests.rs"]
mod lightbar_tests;
#[path = "microphone_tests.rs"]
mod microphone_tests;
#[path = "mode_tests.rs"]
mod mode_tests;
#[path = "multizone_tests.rs"]
mod multizone_tests;
#[path = "mux_tests.rs"]
mod mux_tests;
#[path = "options_tests.rs"]
mod options_tests;
#[path = "perkey_tests.rs"]
mod perkey_tests;
#[path = "platform_tests.rs"]
mod platform_tests;
use ccc_core::msr;
use fixtures::Fixture;

fn target() -> powerctl::runtime::Settings {
    powerctl::runtime::Settings {
        pl1_w: Some(80.125),
        pl2_w: Some(120.0),
        core_offset_units: Some(-70),
        gt_offset_units: Some(-30),
    }
}
fn settings() -> ProfileSettings {
    ProfileSettings {
        runtime_cpu: Some(target()),
        ..Default::default()
    }
}

#[test]
fn manual_review_reports_live_cpu_values_and_blocks_locked_targets_without_outputs() {
    let f = Fixture::new();
    let settings = settings();
    let review = review::preview_manual(&f.hal, &f.gpu, f.devices(), &settings).unwrap();
    assert!(review.blocked.is_none());
    let before = review.before.unwrap().runtime_cpu.unwrap();
    assert_eq!(before.core_offset_units, Some(-51));
    assert_ne!(before.pl1_w, settings.runtime_cpu.unwrap().pl1_w);

    f.device.0.lock().unwrap().power |= 1 << 63;
    let blocked = review::preview_manual(&f.hal, &f.gpu, f.devices(), &settings).unwrap();
    assert!(blocked.blocked.unwrap().contains("锁定"));
    assert!(f.device.0.lock().unwrap().events.is_empty());
}

#[test]
fn runtime_profile_applies_real_domain_and_user_restore_keeps_first_raw_baseline() {
    let f = Fixture::new();
    let original = f.device.0.lock().unwrap().power;
    let result = apply_settings(&f.hal, &f.gpu, f.devices(), &settings()).unwrap();
    assert!(result.iter().any(|s| s.step == "即时 CPU 调校" && s.ok));
    {
        let state = f.device.0.lock().unwrap();
        assert_eq!(
            msr::decode_power_limits(state.power, 0.125),
            (80.125, 120.0)
        );
        assert_eq!(&state.voltage[..2], &[-70, -30]);
        assert_eq!(state.pages[6][32], 0);
        assert!(!state.events.iter().any(|s| s.starts_with("PAGE")));
    }
    let mut second = settings();
    second.runtime_cpu.as_mut().unwrap().pl1_w = Some(75.0);
    apply_settings(&f.hal, &f.gpu, f.devices(), &second).unwrap();
    assert_eq!(f.hal.recovery().unwrap().runtime_pl, Some(original));
    {
        let _write = f.hal.begin_write().unwrap();
        powerctl::restore(&f.hal).unwrap();
    }
    assert_eq!(f.device.0.lock().unwrap().power, original);
    assert_eq!(&f.device.0.lock().unwrap().voltage[..2], &[-51, -26]);
}

#[test]
fn runtime_lock_or_missing_transport_rejects_all_profile_targets_before_write() {
    let f = Fixture::new();
    f.device.0.lock().unwrap().power |= 1 << 63;
    let mut settings = settings();
    settings.oem_power_mode = Some(2);
    settings.pl1_w = Some(70.0);
    let error = apply_settings(&f.hal, &f.gpu, f.devices(), &settings)
        .err()
        .unwrap();
    assert!(error.to_string().contains("锁定"));
    assert!(f.device.0.lock().unwrap().events.is_empty());
    let no_msr = Hal::test_instance(fixtures::io(), None);
    assert!(
        apply_settings(&no_msr, &f.gpu, f.devices(), &settings)
            .err()
            .unwrap()
            .to_string()
            .contains("隔离测试未配置")
    );
    assert!(f.device.0.lock().unwrap().events.is_empty());
}

#[test]
fn unchanged_locked_cpu_allows_other_domains_without_cpu_target_writes() {
    let f = Fixture::new();
    f.device.0.lock().unwrap().power |= 1 << 63;
    let settings = ProfileSettings {
        runtime_cpu: Some(powerctl::runtime::Settings {
            pl1_w: Some(66.125),
            pl2_w: Some(100.125),
            core_offset_units: Some(-51),
            gt_offset_units: Some(-26),
        }),
        gpu: Some(crate::gpuctl::GpuClockSettings {
            core_offset_mhz: Some(60),
            mem_offset_mhz: None,
        }),
        ..Default::default()
    };
    let result = apply_settings(&f.hal, &f.gpu, f.devices(), &settings).unwrap();
    assert!(result.iter().all(|s| s.ok));
    {
        let _write = f.hal.begin_write().unwrap();
        powerctl::restore(&f.hal).unwrap();
    }
    let state = f.device.0.lock().unwrap();
    assert!(state.events.iter().any(|s| s == "GPU:60000"));
    assert!(
        !state
            .events
            .iter()
            .any(|s| s.starts_with("PL:") || s.starts_with("UV:"))
    );
}

#[test]
fn later_gpu_failure_restores_saved_flag_mode_then_exact_runtime_snapshot() {
    let f = Fixture::new();
    f.device.0.lock().unwrap().pages[1][16] = 1;
    let original = f.device.0.lock().unwrap().power;
    f.device.0.lock().unwrap().fail_gpu = true;
    let mut settings = settings();
    settings.oem_power_mode = Some(2);
    settings.pl1_w = Some(70.0);
    settings.gpu = Some(crate::gpuctl::GpuClockSettings {
        core_offset_mhz: Some(60),
        mem_offset_mhz: None,
    });
    let steps = apply_settings(&f.hal, &f.gpu, f.devices(), &settings).unwrap();
    assert!(steps.iter().any(|s| s.step == "即时 CPU 调校" && s.ok));
    assert!(steps.iter().any(|s| s.step == "GPU 偏移" && !s.ok));
    assert!(steps.iter().any(|s| s.step == "恢复即时 CPU" && s.ok));
    let state = f.device.0.lock().unwrap();
    assert_eq!(state.power, original);
    assert_eq!(&state.voltage[..2], &[-51, -26]);
    assert_eq!(state.pages[6][32], 0);
    assert_eq!(state.pages[6][33..37], 65i32.to_le_bytes());
    assert_eq!(state.pages[1][1], 3);
    assert_eq!(state.pages[1][16], 1);
    assert_eq!(state.gpu_khz, 135_000);
    let mode = state.events.iter().rposition(|s| s == "MODE:3").unwrap();
    let voltage = state.events.iter().rposition(|s| s == "UV:0:-51").unwrap();
    assert!(mode < voltage);
}

#[test]
fn ignored_runtime_voltage_stops_later_domains_and_restores_partial_cpu_write() {
    let f = Fixture::new();
    let original = f.device.0.lock().unwrap().power;
    f.device.0.lock().unwrap().ignore_gt_target = true;
    let mut settings = settings();
    settings.gpu = Some(crate::gpuctl::GpuClockSettings {
        core_offset_mhz: Some(60),
        mem_offset_mhz: None,
    });
    let steps = apply_settings(&f.hal, &f.gpu, f.devices(), &settings).unwrap();
    assert!(
        steps
            .iter()
            .any(|s| s.step == "即时 CPU 调校" && !s.ok && s.message.contains("回读不一致"))
    );
    let state = f.device.0.lock().unwrap();
    assert_eq!(state.power, original);
    assert_eq!(&state.voltage[..2], &[-51, -26]);
    assert!(!state.events.iter().any(|s| s.starts_with("GPU:")));
}

#[test]
fn failed_runtime_restore_is_reported_and_user_retry_retains_exact_baseline() {
    let f = Fixture::new();
    let original = f.device.0.lock().unwrap().power;
    {
        let mut state = f.device.0.lock().unwrap();
        state.ignore_gt_target = true;
        state.fail_restore_pl = true;
    }
    let steps = apply_settings(&f.hal, &f.gpu, f.devices(), &settings()).unwrap();
    assert!(
        steps
            .iter()
            .any(|s| s.step == "恢复即时 CPU" && !s.ok && s.message.contains("injected"))
    );
    assert_eq!(&f.device.0.lock().unwrap().voltage[..2], &[-51, -26]);
    assert_eq!(f.hal.recovery().unwrap().runtime_pl, Some(original));
    f.device.0.lock().unwrap().fail_restore_pl = false;
    {
        let _write = f.hal.begin_write().unwrap();
        powerctl::restore(&f.hal).unwrap();
    }
    assert_eq!(f.device.0.lock().unwrap().power, original);
    assert!(f.hal.recovery().unwrap().runtime_pl.is_none());
}

#[test]
fn capture_and_json_keep_runtime_precision_separate_from_saved_values() {
    let f = Fixture::new();
    let captured = crate::profiles::capture_current(
        &f.hal,
        &f.gpu,
        f.devices(),
        &crate::profiles::ProfileGroup::ALL,
    )
    .unwrap();
    assert_eq!(captured.pl1_w, Some(65.0));
    let runtime = captured.runtime_cpu.unwrap();
    assert_eq!(runtime.pl1_w, Some(66.125));
    assert_eq!(runtime.core_offset_units, Some(-51));
    assert_eq!(runtime.gt_offset_units, Some(-26));
    let json = serde_json::to_string(&captured).unwrap();
    let decoded: ProfileSettings = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.runtime_cpu, Some(runtime));
    assert!(f.device.0.lock().unwrap().events.is_empty());
}

#[test]
fn saved_settings_restore_the_original_trigger_last_across_multiple_edits() {
    let f = Fixture::new();
    let _write = f.hal.begin_write().unwrap();
    powerctl::apply_saved(
        &f.hal,
        powerctl::CpuControls {
            pl1_w: Some(70.0),
            ..Default::default()
        },
    )
    .unwrap();
    powerctl::apply_saved(
        &f.hal,
        powerctl::CpuControls {
            gt_offset_mv: Some(-20),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(f.device.0.lock().unwrap().pages[6][32], 1);
    f.device.0.lock().unwrap().events.clear();
    powerctl::restore(&f.hal).unwrap();
    let state = f.device.0.lock().unwrap();
    assert_eq!(state.pages[6][33..37], 65i32.to_le_bytes());
    assert_eq!(state.pages[6][69..73], 0i32.to_le_bytes());
    assert_eq!(state.pages[6][32], 0);
    assert_eq!(state.events.last().unwrap(), "PAGE6:32:[0]");
}

#[test]
fn changed_profile_cannot_execute_an_unreviewed_runtime_target() {
    let f = Fixture::new();
    let reviewed = crate::profiles::ProfileFile {
        version: 1,
        name: "测试档案".into(),
        saved_at_ms: 10,
        settings: settings(),
    };
    let mut current = reviewed.clone();
    current
        .settings
        .runtime_cpu
        .as_mut()
        .unwrap()
        .core_offset_units = Some(-90);
    assert!(
        apply_reviewed(
            &f.hal,
            &f.gpu,
            f.devices(),
            &current,
            &reviewed,
            None,
            None,
            None
        )
        .err()
        .unwrap()
        .to_string()
        .contains("重新预览")
    );
    assert!(f.device.0.lock().unwrap().events.is_empty());
    current = reviewed.clone();
    current.saved_at_ms = 11;
    assert!(
        apply_reviewed(
            &f.hal,
            &f.gpu,
            f.devices(),
            &current,
            &reviewed,
            None,
            None,
            None
        )
        .is_err()
    );
    assert!(f.device.0.lock().unwrap().events.is_empty());
    assert!(
        apply_reviewed(
            &f.hal,
            &f.gpu,
            f.devices(),
            &reviewed,
            &reviewed,
            None,
            None,
            None
        )
        .unwrap()
        .iter()
        .all(|s| s.ok)
    );
    assert_eq!(&f.device.0.lock().unwrap().voltage[..2], &[-70, -30]);
}

#[test]
fn saved_only_profile_keeps_its_original_meaning_without_an_msr_provider() {
    let f = Fixture::new();
    let hal = Hal::test_instance(fixtures::io(), None);
    let before = f.device.0.lock().unwrap().power;
    let settings = ProfileSettings {
        pl1_w: Some(70.0),
        ..Default::default()
    };
    let result = apply_settings(&hal, &f.gpu, f.devices(), &settings).unwrap();
    assert!(result.iter().all(|s| s.ok));
    assert!(!result.iter().any(|s| s.step == "即时 CPU 调校"));
    let state = f.device.0.lock().unwrap();
    assert_eq!(state.power, before);
    assert_eq!(state.pages[6][33..37], 70i32.to_le_bytes());
    assert!(
        !state
            .events
            .iter()
            .any(|s| s.starts_with("PL:") || s.starts_with("UV:"))
    );
}

#[test]
fn running_dust_rejects_all_profile_fan_targets_before_cpu_or_gpu_writes() {
    let f = Fixture::new();
    {
        let mut state = f.device.0.lock().unwrap();
        state.bios[0] = 1;
        state.bios[10] |= 128;
        state.dust_running = true;
    }
    for fan_mode in [Some(0), Some(6)] {
        let target = ProfileSettings {
            fan_mode,
            pl1_w: Some(70.0),
            ..Default::default()
        };
        assert!(
            apply_settings(&f.hal, &f.gpu, f.devices(), &target)
                .err()
                .unwrap()
                .to_string()
                .contains("除尘")
        );
    }
    assert!(f.device.0.lock().unwrap().events.is_empty());
}

#[test]
fn console_runtime_applies_all_changes_as_one_recoverable_transaction() {
    let f = Fixture::new();
    let before = f.device.0.lock().unwrap().power;
    f.device.0.lock().unwrap().ignore_gt_target = true;
    let controls = powerctl::CpuControls {
        pl1_w: Some(80.125),
        pl2_w: Some(120.0),
        core_offset_mv: Some(-68),
        gt_offset_mv: Some(-29),
    };
    let _write = f.hal.begin_write().unwrap();
    let error = powerctl::apply_runtime(&f.hal, controls)
        .unwrap_err()
        .to_string();
    assert!(error.contains("回读不一致") && error.contains("已恢复"));
    let state = f.device.0.lock().unwrap();
    assert_eq!(state.power, before);
    assert_eq!(&state.voltage[..2], &[-51, -26]);
}

#[test]
fn console_saved_partial_write_restores_all_fields_and_original_flag() {
    let f = Fixture::new();
    let before = f.device.0.lock().unwrap().pages[6];
    f.device.0.lock().unwrap().fail_saved = true;
    let controls = powerctl::CpuControls {
        pl1_w: Some(70.0),
        core_offset_mv: Some(-60),
        gt_offset_mv: Some(-20),
        ..Default::default()
    };
    let _write = f.hal.begin_write().unwrap();
    assert!(
        powerctl::apply_saved(&f.hal, controls)
            .unwrap_err()
            .to_string()
            .contains("已恢复")
    );
    assert_eq!(f.device.0.lock().unwrap().pages[6], before);
}

#[test]
fn early_saved_failure_keeps_cpu_baseline_when_mode_changed_it_and_restore_failed() {
    let f = Fixture::new();
    let original = f.device.0.lock().unwrap().power;
    {
        let mut state = f.device.0.lock().unwrap();
        state.fail_saved = true;
        state.fail_restore_pl = true;
    }
    let mut wanted = settings();
    wanted.oem_power_mode = Some(2);
    wanted.gt_offset_mv = Some(-20);
    let steps = apply_settings(&f.hal, &f.gpu, f.devices(), &wanted).unwrap();
    assert!(!steps.iter().any(|s| s.step == "即时 CPU 调校"));
    assert!(steps.iter().any(|s| s.step == "恢复即时 CPU" && !s.ok));
    assert_eq!(f.hal.recovery().unwrap().runtime_pl, Some(original));
    f.device.0.lock().unwrap().fail_restore_pl = false;
    let _write = f.hal.begin_write().unwrap();
    powerctl::restore(&f.hal).unwrap();
    assert_eq!(f.device.0.lock().unwrap().power, original);
    assert_eq!(&f.device.0.lock().unwrap().voltage[..2], &[-51, -26]);
}

#[test]
fn failed_mode_command_and_compensation_keep_user_baseline_for_retry() {
    let f = Fixture::new();
    let _write = f.hal.begin_write().unwrap();
    f.device.0.lock().unwrap().fail_mode = true;
    let error = powerctl::mode::checkpoint(&f.hal, 2)
        .unwrap()
        .execute(&f.hal)
        .unwrap_err()
        .to_string();
    assert!(error.contains("恢复失败"));
    assert!(f.hal.recovery().unwrap().oem_mode.is_some());
    f.device.0.lock().unwrap().fail_mode = false;
    powerctl::mode::checkpoint(&f.hal, 2)
        .unwrap()
        .execute(&f.hal)
        .unwrap();
    assert_eq!(f.device.0.lock().unwrap().pages[1][1], 2);
}

#[path = "touchpad_tests.rs"]
mod touchpad_tests;

#[path = "brightness_event_tests.rs"]
mod brightness_event_tests;
#[path = "color_cycle_tests.rs"]
mod color_cycle_tests;
