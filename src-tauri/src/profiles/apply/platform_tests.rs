use super::*;
use crate::{hal::power_plan::SCHEMES, platform as domain};
use serde_json::json;

fn setup(f: &Fixture) {
    let mut state = f.device.0.lock().unwrap();
    state.bios[0] = 1;
    state.bios[23] = 3;
    let mut table = [0; 256];
    table[..3].copy_from_slice(&[1, 0, 100]);
    table[16..20].fill(15);
    state.policy_tables.insert(61, table);
    state.usb_values.insert(SCHEMES[1], [1, 0]);
    state.usb_values.insert(SCHEMES[2], [0, 0]);
}
fn wanted() -> ProfileSettings {
    serde_json::from_value(json!({
        "powerDelivery":{"level":4,"encoding":"adapterV1","adapterWatts":100,"availableLevels":[1,2,3,4]},
        "usbPolicy":{"plans":[{"id":"balanced","ac":true,"dc":false},
            {"id":"highPerformance","ac":false,"dc":true},{"id":"powerSaver","ac":true,"dc":null}]}
    })).unwrap()
}
fn apply(f: &Fixture, settings: &ProfileSettings) -> Vec<ApplyStep> {
    apply_settings(&f.hal, &f.gpu, f.devices(), settings).unwrap()
}

#[test]
fn platform_capture_and_strict_file_roundtrip_preserve_encoding_and_heterogeneous_usb_fields() {
    let f = Fixture::new();
    setup(&f);
    f.device
        .0
        .lock()
        .unwrap()
        .usb_values
        .get_mut(&SCHEMES[2])
        .unwrap()[1] = 99;
    let capture = crate::profiles::capture_current(&f.hal, &f.gpu, f.devices()).unwrap();
    let target = capture.power_delivery.as_ref().unwrap();
    assert_eq!(target.level, 1);
    assert_eq!(
        serde_json::to_value(target).unwrap()["encoding"],
        "adapterV1"
    );
    let plans = &capture.usb_policy.as_ref().unwrap().plans;
    assert_eq!((plans[0].ac, plans[0].dc), (Some(false), Some(true)));
    assert_eq!((plans[1].ac, plans[1].dc), (Some(true), Some(false)));
    assert_eq!((plans[2].ac, plans[2].dc), (Some(false), None));
    let dir = tempfile::tempdir().unwrap();
    crate::profiles::save_in(dir.path(), capture.clone(), "平台").unwrap();
    assert!(
        crate::profiles::load_from(dir.path(), "平台")
            .unwrap()
            .settings
            == capture
    );
    assert!(f.device.0.lock().unwrap().events.is_empty());
}

#[test]
fn profile_platform_success_uses_distinct_plans_and_keeps_unselected_fields() {
    let f = Fixture::new();
    setup(&f);
    let steps = apply(&f, &wanted());
    assert!(steps.iter().all(|step| step.ok));
    assert_eq!(
        steps
            .iter()
            .map(|step| step.step.as_str())
            .collect::<Vec<_>>(),
        ["PD档位", "USB选择性暂停"]
    );
    let state = f.device.0.lock().unwrap();
    assert_eq!(state.pd_level[0], 4);
    assert_eq!(state.usb_values[&SCHEMES[0]], [1, 0]);
    assert_eq!(state.usb_values[&SCHEMES[1]], [0, 1]);
    assert_eq!(state.usb_values[&SCHEMES[2]], [1, 0]);
    assert_eq!(state.active_plan, SCHEMES[0]);
}

#[test]
fn later_gpu_failure_restores_pd_and_only_attempted_usb_fields() {
    let f = Fixture::new();
    setup(&f);
    let original = f.device.0.lock().unwrap().usb_values.clone();
    f.device.0.lock().unwrap().fail_gpu = true;
    let mut settings = wanted();
    settings.gpu = Some(crate::gpuctl::GpuClockSettings {
        core_offset_mhz: Some(60),
        mem_offset_mhz: None,
    });
    let steps = apply(&f, &settings);
    assert!(steps.iter().any(|step| step.step == "GPU 偏移" && !step.ok));
    assert!(
        steps
            .iter()
            .any(|step| step.step == "恢复USB策略" && step.ok)
    );
    assert!(
        steps
            .iter()
            .any(|step| step.step == "恢复PD档位" && step.ok)
    );
    let state = f.device.0.lock().unwrap();
    assert_eq!(state.pd_level[0], 1);
    assert_eq!(state.usb_values, original);
}

#[test]
fn invalid_platform_target_or_preflight_condition_prevents_all_domain_writes() {
    for case in 0..6 {
        let f = Fixture::new();
        setup(&f);
        let mut settings = wanted();
        settings.runtime_cpu = Some(target());
        match case {
            0 => settings.power_delivery.as_mut().unwrap().adapter_watts = Some(65),
            1 => settings.power_delivery.as_mut().unwrap().available_levels = vec![1, 4],
            2 => f.device.0.lock().unwrap().bios[23] = 1,
            3 => {
                f.device.0.lock().unwrap().plan_values.remove(&SCHEMES[2]);
            }
            4 => {
                f.device
                    .0
                    .lock()
                    .unwrap()
                    .usb_values
                    .get_mut(&SCHEMES[1])
                    .unwrap()[0] = 99
            }
            _ => {
                let pd = settings.power_delivery.as_mut().unwrap();
                pd.available_levels = vec![4, 1];
            }
        }
        assert!(
            apply_settings(&f.hal, &f.gpu, f.devices(), &settings).is_err(),
            "case {case}"
        );
        assert!(f.device.0.lock().unwrap().events.is_empty(), "case {case}");
    }
}

#[test]
fn pd_encoding_changes_and_adapter_changes_after_prepare_never_write() {
    for adapter in [false, true] {
        let f = Fixture::new();
        setup(&f);
        let mut scope = f.power_delivery.scope().unwrap();
        let _gate = f.hal.begin_write().unwrap();
        let plan = scope
            .prepare(&f.hal, wanted().power_delivery.as_ref().unwrap())
            .unwrap();
        {
            let mut state = f.device.0.lock().unwrap();
            let table = state.policy_tables.get_mut(&61).unwrap();
            if adapter {
                table[2] = 65;
            } else {
                table.fill(0);
            }
        }
        let mut touched = false;
        assert!(scope.execute(&f.hal, &plan, &mut touched).is_err());
        assert!(!touched);
        assert!(f.device.0.lock().unwrap().events.is_empty());
    }
}

#[test]
fn usb_external_plan_or_field_change_after_prepare_never_writes_or_captures_baseline() {
    for active in [false, true] {
        let f = Fixture::new();
        setup(&f);
        let mut scope = f.usb.scope().unwrap();
        let _gate = f.hal.begin_write().unwrap();
        let plan = scope
            .prepare(&f.hal, wanted().usb_policy.as_ref().unwrap(), None)
            .unwrap();
        {
            let mut state = f.device.0.lock().unwrap();
            if active {
                state.active_plan = SCHEMES[2];
            } else {
                state.usb_values.get_mut(&SCHEMES[0]).unwrap()[0] = 1;
            }
        }
        let mut touched = Vec::new();
        assert!(scope.execute(&f.hal, &plan, &mut touched).is_err());
        assert!(touched.is_empty());
        assert!(f.device.0.lock().unwrap().events.is_empty());
    }
}

#[test]
fn mode_authorized_plan_switch_precedes_usb_and_failure_restores_original_mode_plan() {
    for fail_gpu in [false, true] {
        let f = Fixture::new();
        setup(&f);
        super::mode_tests::enable_policy(&f);
        f.device.0.lock().unwrap().fail_gpu = fail_gpu;
        let mut settings = wanted();
        settings.oem_power_mode = Some(4);
        if fail_gpu {
            settings.gpu = Some(crate::gpuctl::GpuClockSettings {
                core_offset_mhz: Some(60),
                mem_offset_mhz: None,
            });
        }
        let steps = apply(&f, &settings);
        assert!(
            steps
                .iter()
                .any(|step| step.step == "USB选择性暂停" && step.ok)
        );
        let state = f.device.0.lock().unwrap();
        assert_eq!(state.active_plan, SCHEMES[if fail_gpu { 0 } else { 2 }]);
        assert_eq!(state.pages[1][1], if fail_gpu { 3 } else { 4 });
        let first_usb = state
            .events
            .iter()
            .position(|event| event.starts_with("USB:"))
            .unwrap();
        let switch = state
            .events
            .iter()
            .position(|event| event == &format!("PLAN:{:x}", SCHEMES[2]))
            .unwrap();
        assert!(switch < first_usb);
    }
}

#[test]
fn pd_single_profile_and_restore_share_the_earliest_baseline() {
    let f = Fixture::new();
    setup(&f);
    let revision = f
        .power_delivery
        .read(&f.hal)
        .unwrap()
        .power_delivery
        .unwrap()
        .revision;
    f.power_delivery
        .apply(
            &f.hal,
            &domain::Request {
                expected_revision: revision,
                level: 2,
            },
        )
        .unwrap();
    assert!(apply(&f, &wanted()).iter().all(|step| step.ok));
    let pd = f
        .power_delivery
        .read(&f.hal)
        .unwrap()
        .power_delivery
        .unwrap();
    assert_eq!(pd.initial.unwrap().value, 1);
    f.power_delivery.restore(&f.hal, &pd.revision).unwrap();
    assert_eq!(f.device.0.lock().unwrap().pd_level[0], 1);
}

#[test]
fn usb_single_profile_and_restore_share_first_field_baselines() {
    let f = Fixture::new();
    setup(&f);
    let before = f.device.0.lock().unwrap().usb_values.clone();
    let revision = f.usb.read(&f.hal).unwrap().unwrap().revision;
    let request =
        serde_json::from_value(json!({"expectedRevision":revision,"values":{"ac":true,"dc":null}}))
            .unwrap();
    f.usb.apply(&f.hal, &request).unwrap();
    assert!(apply(&f, &wanted()).iter().all(|step| step.ok));
    let revision = f.usb.read(&f.hal).unwrap().unwrap().revision;
    f.usb.restore(&f.hal, &revision).unwrap();
    assert_eq!(f.device.0.lock().unwrap().usb_values, before);
}

#[test]
fn partial_platform_failure_compensates_locally_without_duplicate_outer_writes() {
    for pd in [false, true] {
        let f = Fixture::new();
        setup(&f);
        let original = f.device.0.lock().unwrap().usb_values.clone();
        if pd {
            f.device.0.lock().unwrap().fail_pd = 1;
        } else {
            f.device.0.lock().unwrap().fail_usb_at = vec![2];
        }
        let steps = apply(&f, &wanted());
        assert!(steps.iter().any(|step| !step.ok));
        assert!(!steps.iter().any(|step| step.step
            == if pd {
                "恢复PD档位"
            } else {
                "恢复USB策略"
            }));
        let state = f.device.0.lock().unwrap();
        assert_eq!(state.pd_level[0], 1);
        assert_eq!(state.usb_values, original);
    }
}

#[test]
fn usb_failed_local_compensation_is_retried_by_profile_and_initial_restore_remains_available() {
    let f = Fixture::new();
    setup(&f);
    let original = f.device.0.lock().unwrap().usb_values.clone();
    f.device.0.lock().unwrap().fail_usb_at = vec![2, 3];
    let steps = apply(&f, &wanted());
    assert!(
        steps
            .iter()
            .any(|step| step.step == "恢复USB策略" && step.ok)
    );
    assert_eq!(f.device.0.lock().unwrap().usb_values, original);
    assert!(!f.usb.read(&f.hal).unwrap().unwrap().initial.is_empty());
}

#[test]
fn platform_contract_rejects_duplicate_unknown_or_empty_targets_and_keeps_unknown_usb_unselected() {
    let mut settings = wanted();
    let json = serde_json::to_value(&settings).unwrap();
    let mut bad = json.clone();
    bad["powerDelivery"]["legacy"] = json!(true);
    assert!(serde_json::from_value::<ProfileSettings>(bad).is_err());
    let mut bad = json.clone();
    bad["usbPolicy"]["plans"][0]["wake"] = json!(true);
    assert!(serde_json::from_value::<ProfileSettings>(bad).is_err());
    let plans = &mut settings.usb_policy.as_mut().unwrap().plans;
    plans.push(plans[0].clone());
    assert!(crate::profiles::validate(&settings).is_err());
    let mut settings = wanted();
    settings.usb_policy.as_mut().unwrap().plans.clear();
    assert!(crate::profiles::validate(&settings).is_err());
    let f = Fixture::new();
    setup(&f);
    f.device
        .0
        .lock()
        .unwrap()
        .usb_values
        .get_mut(&SCHEMES[2])
        .unwrap()[1] = 99;
    assert!(apply(&f, &wanted()).iter().all(|step| step.ok));
    assert_eq!(f.device.0.lock().unwrap().usb_values[&SCHEMES[2]][1], 99);
}

#[test]
fn usb_controller_without_explicit_test_io_refuses_before_any_native_access() {
    let f = Fixture::new();
    let controller = domain::usb::Controller::default();
    assert!(
        controller
            .read(&f.hal)
            .err()
            .unwrap()
            .to_string()
            .contains("未注入IO")
    );
    assert!(f.device.0.lock().unwrap().events.is_empty());
}

#[test]
fn old_pd_profiles_capture_and_restore_raw_zero_based_or_one_based_numbering() {
    for (raw, maximum, encoding) in [(0, 0, "legacyZeroBased"), (1, 3, "legacyOneBased")] {
        let f = Fixture::new();
        setup(&f);
        {
            let mut state = f.device.0.lock().unwrap();
            state.pd_level = [raw, maximum];
            state.policy_tables.get_mut(&61).unwrap().fill(0);
        }
        let captured = {
            let scope = f.power_delivery.scope().unwrap();
            let _gate = f.hal.begin_write().unwrap();
            scope.capture(&f.hal).unwrap().unwrap()
        };
        let json = serde_json::to_value(&captured).unwrap();
        assert_eq!(json["encoding"], encoding);
        assert_eq!(captured.level, raw);
        assert_eq!(captured.adapter_watts, None);
        let mut target = captured;
        target.level = *target.available_levels.last().unwrap();
        let settings = ProfileSettings {
            power_delivery: Some(target.clone()),
            ..Default::default()
        };
        assert!(apply(&f, &settings).iter().all(|step| step.ok));
        assert_eq!(f.device.0.lock().unwrap().pd_level[0], target.level);
        let revision = f
            .power_delivery
            .read(&f.hal)
            .unwrap()
            .power_delivery
            .unwrap()
            .revision;
        f.power_delivery.restore(&f.hal, &revision).unwrap();
        assert_eq!(f.device.0.lock().unwrap().pd_level[0], raw);
    }
}

#[test]
fn usb_capture_refuses_all_unknown_fields_without_replacing_them_with_defaults() {
    let f = Fixture::new();
    setup(&f);
    f.device
        .0
        .lock()
        .unwrap()
        .usb_values
        .values_mut()
        .for_each(|values| *values = [99, 77]);
    let scope = f.usb.scope().unwrap();
    let _gate = f.hal.begin_write().unwrap();
    assert!(
        scope
            .capture(&f.hal)
            .err()
            .unwrap()
            .to_string()
            .contains("没有已知目标字段")
    );
    assert!(f.device.0.lock().unwrap().events.is_empty());
}
