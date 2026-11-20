use super::*;
use crate::powerctl::mode::boost;
use serde_json::json;
#[path = "options_source_tests.rs"]
mod source;

fn setup() -> Fixture {
    let mut f = Fixture::new();
    f.hal.set_test_dtt_driver(std::sync::Arc::new(|| Ok(true)));
    {
        let mut state = f.device.0.lock().unwrap();
        state.pages[1][1] = 2;
        state.bios[0] = 1;
        state.bios[11] = 16;
        state.bios[16] = 16;
    }
    f
}
fn values(turbo: bool, dtt: bool) -> boost::Values {
    boost::Values {
        turbo_fan: Some(turbo),
        dtt: Some(dtt),
    }
}
fn wanted(turbo: bool, dtt: bool) -> ProfileSettings {
    ProfileSettings {
        performance_options: Some(values(turbo, dtt)),
        ..Default::default()
    }
}
fn apply(f: &Fixture, target: &ProfileSettings) -> Vec<ApplyStep> {
    apply_settings(&f.hal, &f.gpu, f.devices(), target).unwrap()
}
fn gpu_failure(f: &Fixture, target: &mut ProfileSettings) {
    f.device.0.lock().unwrap().fail_gpu = true;
    target.gpu = Some(crate::gpuctl::GpuClockSettings {
        core_offset_mhz: Some(60),
        mem_offset_mhz: None,
    });
}

#[test]
fn options_capture_keeps_inactive_preferences_without_querying_native_drivers() {
    let mut f = setup();
    let queries = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counted = queries.clone();
    f.hal.set_test_dtt_driver(std::sync::Arc::new(move || {
        counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        anyhow::bail!("捕获不应查询驱动")
    }));
    {
        let mut state = f.device.0.lock().unwrap();
        state.pages[1][1] = 3;
        state.pages[4][8] = 1;
        state.pages[1][32] = 1;
    }
    let settings = crate::profiles::capture_current(&f.hal, &f.gpu, f.devices(), &crate::profiles::ProfileGroup::ALL).unwrap();
    assert_eq!(
        settings.performance_options.as_ref().unwrap(),
        &values(true, true)
    );
    assert_eq!(settings.oem_power_mode, Some(3));
    let dir = tempfile::tempdir().unwrap();
    crate::profiles::save_in(dir.path(), settings.clone(), "选项", None).unwrap();
    assert!(
        crate::profiles::load_from(dir.path(), "选项")
            .unwrap()
            .settings
            == settings
    );
    assert_eq!(queries.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert!(f.device.0.lock().unwrap().events.is_empty());
}

#[test]
fn inactive_option_only_profile_saves_flags_without_mode_fan_or_windows_writes() {
    let f = setup();
    f.device.0.lock().unwrap().pages[1][1] = 3;
    assert!(apply(&f, &wanted(true, true)).iter().all(|step| step.ok));
    let state = f.device.0.lock().unwrap();
    assert_eq!(
        (state.pages[4][8], state.pages[1][32], state.pages[1][1]),
        (1, 1, 3)
    );
    assert!(
        state
            .events
            .iter()
            .all(|event| event.starts_with("PAGE4:8:") || event.starts_with("PAGE1:32:"))
    );
}

#[test]
fn dtt_only_profile_keeps_windows_and_fan_while_sending_the_performance_bit() {
    let f = setup();
    let steps = apply(&f, &wanted(false, true));
    assert!(steps.iter().any(|step| step.step == "性能选项" && step.ok));
    let state = f.device.0.lock().unwrap();
    assert!(state.events.iter().any(|event| event == "MODE:130"));
    assert!(
        !state
            .events
            .iter()
            .any(|event| event.starts_with("PAGE4:5:")
                || event.starts_with("PLAN:")
                || event.starts_with("ASPM:"))
    );
}

#[test]
fn combined_mode_preview_and_execution_use_desired_options_in_the_first_command() {
    let f = setup();
    f.device.0.lock().unwrap().pages[1][1] = 3;
    let mut settings = wanted(true, true);
    settings.oem_power_mode = Some(2);
    let preview = {
        let _gate = f.hal.begin_write().unwrap();
        crate::profiles::preview_mode(
            &f.devices().mode_context(&f.hal, &f.gpu).unwrap(),
            &settings,
        )
        .unwrap()
        .unwrap()
    };
    assert!(preview.turbo && preview.dtt);
    assert_eq!(preview.fan_mode, 0);
    assert!(f.device.0.lock().unwrap().events.is_empty());
    assert!(apply(&f, &settings).iter().all(|step| step.ok));
    let state = f.device.0.lock().unwrap();
    let commands: Vec<_> = state
        .events
        .iter()
        .filter(|event| event.starts_with("MODE:"))
        .map(String::as_str)
        .collect();
    assert_eq!(commands, ["MODE:194"]);
    let command = state
        .events
        .iter()
        .position(|event| event == "MODE:194")
        .unwrap();
    assert!(
        state.events[..command]
            .iter()
            .any(|event| event.starts_with("PAGE4:8:"))
    );
    assert!(
        state.events[..command]
            .iter()
            .any(|event| event.starts_with("PAGE1:32:"))
    );
}

#[test]
fn combined_mode_can_disable_cached_dtt_without_activating_it_before_driver_check() {
    let mut f = setup();
    f.hal.set_test_dtt_driver(std::sync::Arc::new(|| Ok(false)));
    {
        let mut state = f.device.0.lock().unwrap();
        state.pages[1][1] = 3;
        state.pages[1][32] = 1;
    }
    let mut settings = wanted(false, false);
    settings.oem_power_mode = Some(2);
    assert!(apply(&f, &settings).iter().all(|step| step.ok));
    let state = f.device.0.lock().unwrap();
    assert_eq!(state.pages[1][32], 0);
    assert_eq!(
        state
            .events
            .iter()
            .filter(|event| event.starts_with("MODE:"))
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["MODE:2"]
    );
}

#[test]
fn turbo_off_precedes_custom_curve_and_on_conflicts_before_any_target() {
    let f = setup();
    f.device.0.lock().unwrap().pages[4][8] = 1;
    let mut settings = wanted(false, false);
    settings.fan_mode = Some(6);
    let steps = apply(&f, &settings);
    assert!(steps.iter().any(|step| step.step == "性能选项" && step.ok));
    assert!(steps.iter().any(|step| step.step == "风扇模式"));
    let change = steps
        .iter()
        .position(|step| step.step == "性能选项")
        .unwrap();
    let fan = steps
        .iter()
        .position(|step| step.step == "风扇模式")
        .unwrap();
    assert!(change < fan);
    assert!(steps[fan].ok, "{}", steps[fan].message);
    assert!(!f.hal.fan_guard_armed().unwrap());
    let count = {
        let state = f.device.0.lock().unwrap();
        assert_eq!((state.pages[4][8], state.pages[4][5]), (0, 6));
        let turbo_off = state
            .events
            .iter()
            .position(|event| event == "PAGE4:8:[0]")
            .unwrap();
        let custom = state
            .events
            .iter()
            .position(|event| event == "PAGE4:5:[6]")
            .unwrap();
        assert!(turbo_off < custom);
        state.events.len()
    };
    settings.performance_options = Some(values(true, false));
    assert!(apply_settings(&f.hal, &f.gpu, f.devices(), &settings).is_err());
    assert_eq!(f.device.0.lock().unwrap().events.len(), count);
}

#[test]
fn later_failure_restores_option_only_flags_without_restoring_windows() {
    let f = setup();
    let mut settings = wanted(false, true);
    gpu_failure(&f, &mut settings);
    let steps = apply(&f, &settings);
    assert!(
        steps
            .iter()
            .any(|step| step.step == "恢复性能选项" && step.ok)
    );
    let state = f.device.0.lock().unwrap();
    assert_eq!((state.pages[4][8], state.pages[1][32]), (0, 0));
    assert!(
        !state
            .events
            .iter()
            .any(|event| event.starts_with("PLAN:") || event.starts_with("ASPM:"))
    );
}

#[test]
fn combined_mode_gpu_failure_restores_mode_preferences_and_runtime_cpu_in_dependency_order() {
    let f = setup();
    f.device.0.lock().unwrap().pages[1][1] = 3;
    let power = f.device.0.lock().unwrap().power;
    let mut settings = wanted(true, true);
    settings.oem_power_mode = Some(2);
    settings.runtime_cpu = Some(target());
    gpu_failure(&f, &mut settings);
    let steps = apply(&f, &settings);
    assert!(
        steps
            .iter()
            .any(|step| step.step == "恢复 OEM 电源模式" && step.ok)
    );
    let state = f.device.0.lock().unwrap();
    assert_eq!(
        (state.pages[1][1], state.pages[4][8], state.pages[1][32]),
        (3, 0, 0)
    );
    assert_eq!(state.power, power);
    let mode = steps
        .iter()
        .position(|step| step.step == "恢复 OEM 电源模式")
        .unwrap();
    let cpu = steps
        .iter()
        .position(|step| step.step == "恢复即时 CPU")
        .unwrap();
    assert!(mode < cpu);
}

#[test]
fn partial_flag_save_recovers_locally_and_retains_shared_initial_restore() {
    let f = setup();
    f.device.0.lock().unwrap().fail_option_save = Some((1, 32));
    let steps = apply(&f, &wanted(true, true));
    assert!(steps.iter().any(|step| step.step == "性能选项" && !step.ok));
    assert!(!steps.iter().any(|step| step.step == "恢复性能选项"));
    assert!(f.hal.recovery().unwrap().oem_mode.is_some());
    let state = f.device.0.lock().unwrap();
    assert_eq!((state.pages[4][8], state.pages[1][32]), (0, 0));
}

#[test]
fn partial_restore_continues_other_flags_and_can_retry_the_same_checkpoint() {
    let f = setup();
    let _gate = f.hal.begin_write().unwrap();
    let plan = boost::Plan::prepare(&f.hal, &values(true, true)).unwrap();
    let mut change = boost::Change::default();
    plan.execute(&f.hal, false, &mut change).unwrap();
    f.device.0.lock().unwrap().fail_option_save = Some((4, 8));
    assert!(plan.rollback(&f.hal, &mut change, true).is_err());
    assert_eq!(f.device.0.lock().unwrap().pages[1][32], 0);
    plan.rollback(&f.hal, &mut change, true).unwrap();
    assert!(!change.dirty());
    assert_eq!(f.device.0.lock().unwrap().pages[4][8], 0);
}

#[test]
fn single_and_profile_options_share_the_earliest_mode_baseline() {
    let f = setup();
    {
        let _gate = f.hal.begin_write().unwrap();
        let profile = boost::read(&f.hal).unwrap();
        boost::apply(
            &f.hal,
            &boost::Request {
                expected_revision: profile.revision,
                quiet_fan: None,
                replace_custom_fan: false,
                values: values(false, true),
            },
        )
        .unwrap();
    }
    assert!(apply(&f, &wanted(true, false)).iter().all(|step| step.ok));
    let stored = f.hal.recovery().unwrap().oem_mode.clone().unwrap().stored();
    let preview = stored.recovery_preview(&stored).unwrap();
    assert_eq!(
        (preview.initial_mode, preview.initial_dtt),
        (2, false)
    );
}

#[test]
fn empty_unknown_or_incompatible_options_are_never_accepted_as_a_profile() {
    let mut settings = wanted(false, false);
    settings.performance_options = Some(boost::Values {
        turbo_fan: None,
        dtt: None,
    });
    assert!(crate::profiles::validate(&settings).is_err());
    let mut bad = serde_json::to_value(wanted(false, false)).unwrap();
    bad["performanceOptions"]["cpuBoost"] = json!(true);
    assert!(serde_json::from_value::<ProfileSettings>(bad).is_err());
    let f = setup();
    f.device.0.lock().unwrap().bios[16] = 0;
    assert!(apply_settings(&f.hal, &f.gpu, f.devices(), &wanted(false, false)).is_err());
    assert!(f.device.0.lock().unwrap().events.is_empty());
}
