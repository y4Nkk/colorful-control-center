use super::*;
use crate::fanctl::dust::Schedule;

fn enabled() -> Schedule {
    Schedule {
        weekdays: 65,
        hour: 23,
        on_resume: true,
    }
}
fn disabled() -> Schedule {
    Schedule {
        weekdays: 0,
        hour: 0,
        on_resume: false,
    }
}
fn setup(f: &Fixture, schedule: Option<Schedule>) {
    let mut state = f.device.0.lock().unwrap();
    state.bios[0] = 1;
    state.bios[10] = 128;
    state.pages[4][81..86].copy_from_slice(&schedule.map_or([0, 17, 18, 19, 20], |s| s.saved()));
    state.pages[4][80] = 72;
    state.pages[4][86] = 73;
}
fn curves() -> ccc_core::software_fan::Curves {
    serde_json::from_value(
        serde_json::json!({"cpu": {"points": [{"tempC":40,"dutyPct":20},
        {"tempC":95,"dutyPct":100}],"hysteresisC":3,"rampPctPerS":10}}),
    )
    .unwrap()
}

#[test]
fn captured_dust_plan_roundtrips_and_unknown_original_is_never_fabricated() {
    let f = Fixture::new();
    setup(&f, Some(enabled()));
    let captured = crate::profiles::capture_current(&f.hal, &f.gpu, f.devices(), &crate::profiles::ProfileGroup::ALL).unwrap();
    assert_eq!(captured.dust_schedule, Some(enabled()));
    let dir = tempfile::tempdir().unwrap();
    crate::profiles::save_in(dir.path(), captured, "除尘", None).unwrap();
    assert_eq!(
        crate::profiles::load_from(dir.path(), "除尘")
            .unwrap()
            .settings
            .dust_schedule,
        Some(enabled())
    );
    setup(&f, None);
    assert!(
        crate::profiles::capture_current(&f.hal, &f.gpu, f.devices(), &crate::profiles::ProfileGroup::ALL)
            .err()
            .unwrap()
            .to_string()
            .contains("未报告")
    );
    assert!(f.device.0.lock().unwrap().events.is_empty());
}

#[test]
fn dust_only_profile_is_valid_and_conflicting_software_target_fails_before_all_writes() {
    let f = Fixture::new();
    setup(&f, Some(disabled()));
    let mut settings = ProfileSettings {
        dust_schedule: Some(enabled()),
        ..Default::default()
    };
    crate::profiles::validate(&settings).unwrap();
    settings.fan_software_curves = Some(curves());
    assert!(apply_settings(&f.hal, &f.gpu, f.devices(), &settings).is_err());
    settings.fan_software_curves = None;
    settings.dust_schedule.as_mut().unwrap().hour = 24;
    assert!(apply_settings(&f.hal, &f.gpu, f.devices(), &settings).is_err());
    assert!(f.device.0.lock().unwrap().events.is_empty());
}

#[test]
fn later_gpu_failure_restores_raw_schedule_and_retains_shared_initial_state() {
    let f = Fixture::new();
    setup(&f, Some(enabled()));
    f.device.0.lock().unwrap().fail_gpu = true;
    let settings = ProfileSettings {
        dust_schedule: Some(disabled()),
        gpu: Some(crate::gpuctl::GpuClockSettings {
            core_offset_mhz: Some(60),
            mem_offset_mhz: None,
        }),
        ..settings()
    };
    let steps = apply_settings(&f.hal, &f.gpu, f.devices(), &settings).unwrap();
    assert!(steps.iter().any(|s| s.step == "除尘计划" && s.ok));
    assert!(steps.iter().any(|s| s.step == "恢复除尘计划" && s.ok));
    {
        let state = f.device.0.lock().unwrap();
        assert_eq!(&state.pages[4][81..86], &enabled().saved());
        assert_eq!((state.pages[4][80], state.pages[4][86]), (72, 73));
        assert!(!state.events.iter().any(|s| s.starts_with("DUST:41:")));
    }
    let profile = serde_json::to_value(f.dust.read(&f.hal).unwrap().unwrap()).unwrap();
    assert!(!profile["initial"].is_null());
    f.dust
        .apply(
            &f.hal,
            &fanctl::dust::Request {
                expected_revision: profile["revision"].as_str().unwrap().into(),
                schedule: disabled(),
            },
        )
        .unwrap();
    let profile = serde_json::to_value(f.dust.read(&f.hal).unwrap().unwrap()).unwrap();
    f.dust
        .restore(&f.hal, profile["revision"].as_str().unwrap())
        .unwrap();
    assert_eq!(
        &f.device.0.lock().unwrap().pages[4][81..86],
        &enabled().saved()
    );
}

#[test]
fn partial_schedule_save_self_compensates_without_duplicate_outer_recovery() {
    let f = Fixture::new();
    setup(&f, Some(enabled()));
    f.device.0.lock().unwrap().fail_dust_save = true;
    let settings = ProfileSettings {
        dust_schedule: Some(disabled()),
        ..settings()
    };
    let steps = apply_settings(&f.hal, &f.gpu, f.devices(), &settings).unwrap();
    assert!(
        steps
            .iter()
            .any(|s| s.step == "除尘计划" && !s.ok && s.message.contains("已恢复"))
    );
    assert!(
        !steps
            .iter()
            .any(|s| s.step == "即时 CPU 调校" || s.step == "恢复除尘计划")
    );
    assert_eq!(
        &f.device.0.lock().unwrap().pages[4][81..86],
        &enabled().saved()
    );
}

#[test]
fn failed_local_schedule_recovery_retries_globally_and_unknown_raw_initial_survives() {
    let f = Fixture::new();
    setup(&f, None);
    let original = f.device.0.lock().unwrap().pages[4][81..86].to_vec();
    f.device.0.lock().unwrap().fail_dust_commands = 2;
    let settings = ProfileSettings {
        dust_schedule: Some(disabled()),
        ..Default::default()
    };
    let steps = apply_settings(&f.hal, &f.gpu, f.devices(), &settings).unwrap();
    assert!(
        steps
            .iter()
            .any(|s| s.step == "除尘计划" && !s.ok && s.message.contains("恢复失败"))
    );
    assert!(steps.iter().any(|s| s.step == "恢复除尘计划" && s.ok));
    assert_eq!(&f.device.0.lock().unwrap().pages[4][81..86], &original);
    let profile = serde_json::to_value(f.dust.read(&f.hal).unwrap().unwrap()).unwrap();
    assert!(!profile["initial"].is_null());
}

#[test]
fn running_or_missing_dust_capability_rejects_profile_before_cpu_write() {
    let f = Fixture::new();
    let settings = ProfileSettings {
        dust_schedule: Some(disabled()),
        ..settings()
    };
    assert!(apply_settings(&f.hal, &f.gpu, f.devices(), &settings).is_err());
    setup(&f, Some(enabled()));
    f.device.0.lock().unwrap().dust_running = true;
    assert!(apply_settings(&f.hal, &f.gpu, f.devices(), &settings).is_err());
    assert!(f.device.0.lock().unwrap().events.is_empty());
}

#[test]
fn disabling_schedule_precedes_software_takeover_and_failed_guard_restores_previous_plan() {
    let f = Fixture::new();
    setup(&f, Some(enabled()));
    let settings = ProfileSettings {
        dust_schedule: Some(disabled()),
        fan_software_curves: Some(curves()),
        ..Default::default()
    };
    let steps = apply_settings(&f.hal, &f.gpu, f.devices(), &settings).unwrap();
    assert!(steps.iter().any(|s| s.step == "除尘计划" && s.ok));
    assert!(
        steps
            .iter()
            .any(|s| s.step == "软件风扇曲线" && !s.ok && s.message.contains("未启动生产监护"))
    );
    assert!(steps.iter().any(|s| s.step == "恢复除尘计划" && s.ok));
    let fan_restore = steps.iter().position(|s| s.step == "恢复风扇").unwrap();
    let dust_restore = steps.iter().position(|s| s.step == "恢复除尘计划").unwrap();
    assert!(fan_restore < dust_restore);
    assert_eq!(
        &f.device.0.lock().unwrap().pages[4][81..86],
        &enabled().saved()
    );
}

#[test]
fn enabling_plan_requires_explicit_exit_from_active_software_and_recovers_disabled_before_resume() {
    let f = Fixture::new();
    setup(&f, Some(disabled()));
    fanctl::software::inject_test_session(&f.hal, curves()).unwrap();
    let mut settings = ProfileSettings {
        dust_schedule: Some(enabled()),
        ..Default::default()
    };
    assert!(apply_settings(&f.hal, &f.gpu, f.devices(), &settings).is_err());
    assert!(f.device.0.lock().unwrap().events.is_empty());
    settings.fan_mode = Some(0);
    settings.gpu = Some(crate::gpuctl::GpuClockSettings {
        core_offset_mhz: Some(60),
        mem_offset_mhz: None,
    });
    f.device.0.lock().unwrap().fail_gpu = true;
    let steps = apply_settings(&f.hal, &f.gpu, f.devices(), &settings).unwrap();
    assert!(steps.iter().any(|s| s.step == "除尘计划" && s.ok));
    let dust = steps
        .iter()
        .position(|s| s.step == "恢复除尘计划" && s.ok)
        .unwrap();
    let fan = steps.iter().position(|s| s.step == "恢复风扇").unwrap();
    assert!(dust < fan);
    assert!(!steps[fan].ok);
    assert!(steps[fan].message.contains("未启动生产监护"));
    assert_eq!(
        &f.device.0.lock().unwrap().pages[4][81..86],
        &disabled().saved()
    );
}

#[test]
fn prepared_schedule_rechecks_saved_fields_before_write_and_preserves_initial_state() {
    let f = Fixture::new();
    setup(&f, Some(enabled()));
    let mut scope = f.dust.scope().unwrap();
    let _gate = f.hal.begin_write().unwrap();
    let plan = scope.prepare(&f.hal, disabled(), false).unwrap();
    f.device.0.lock().unwrap().pages[4][83] = 11;
    assert!(
        scope
            .execute(&f.hal, &plan, &mut false)
            .unwrap_err()
            .to_string()
            .contains("已变化")
    );
    assert!(f.device.0.lock().unwrap().events.is_empty());
}

#[test]
fn standalone_and_profile_schedule_transactions_share_lock_order_and_first_baseline() {
    let f = Fixture::new();
    setup(&f, Some(enabled()));
    let start = std::sync::Barrier::new(2);
    std::thread::scope(|threads| {
        threads.spawn(|| {
            start.wait();
            for _ in 0..20 {
                let settings = ProfileSettings {
                    dust_schedule: Some(disabled()),
                    ..Default::default()
                };
                assert!(
                    apply_settings(&f.hal, &f.gpu, f.devices(), &settings)
                        .unwrap()
                        .iter()
                        .all(|s| s.ok)
                );
            }
        });
        threads.spawn(|| {
            start.wait();
            for _ in 0..20 {
                let profile = serde_json::to_value(f.dust.read(&f.hal).unwrap().unwrap()).unwrap();
                let result = f.dust.apply(
                    &f.hal,
                    &fanctl::dust::Request {
                        expected_revision: profile["revision"].as_str().unwrap().into(),
                        schedule: enabled(),
                    },
                );
                if let Err(error) = result {
                    assert!(error.to_string().contains("预览已变化"));
                }
            }
        });
    });
    let profile = serde_json::to_value(f.dust.read(&f.hal).unwrap().unwrap()).unwrap();
    f.dust
        .restore(&f.hal, profile["revision"].as_str().unwrap())
        .unwrap();
    assert_eq!(
        &f.device.0.lock().unwrap().pages[4][81..86],
        &enabled().saved()
    );
}
