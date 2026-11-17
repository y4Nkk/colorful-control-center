use super::*;
use std::sync::{Arc, atomic::Ordering};
#[path = "gpu_application_tests.rs"]
mod application;
#[path = "gpu_process_tests.rs"]
mod process;

fn setup(enabled: bool) -> Fixture {
    let mut f = Fixture::new();
    f.enable_gpu_power();
    {
        let mut data = f.gpu_environment.io.0.lock().unwrap();
        data.device.enabled = enabled;
        data.device.problem = if enabled { 0 } else { 22 };
        data.device.automatic_power_off = !enabled;
        data.firmware = enabled;
    }
    f
}
fn wanted(f: &Fixture, enabled: bool) -> ProfileSettings {
    ProfileSettings {
        gpu_power: Some(f.gpu_target(enabled)),
        ..Default::default()
    }
}
fn apply(f: &Fixture, settings: &ProfileSettings) -> Vec<ApplyStep> {
    apply_settings(&f.hal, &f.gpu, f.devices(), settings).unwrap()
}
fn success(steps: &[ApplyStep]) {
    assert!(
        steps.iter().all(|step| step.ok),
        "{}",
        steps
            .iter()
            .map(|s| format!("{}:{}", s.step, s.message))
            .collect::<Vec<_>>()
            .join("；")
    );
}
fn curves() -> ccc_core::software_fan::Curves {
    ["cpu", "gpu1"]
        .into_iter()
        .map(|id| {
            (
                id.into(),
                ccc_core::fan::FanCurve {
                    points: vec![
                        ccc_core::fan::CurvePoint {
                            temp_c: 40.0,
                            duty_pct: 30.0,
                        },
                        ccc_core::fan::CurvePoint {
                            temp_c: 95.0,
                            duty_pct: 100.0,
                        },
                    ],
                    hysteresis_c: 3.0,
                    ramp_pct_per_s: 20.0,
                },
            )
        })
        .collect()
}
#[test]
fn capture_preserves_off_mixed_policy_and_never_reads_disabled_gpu() {
    let f = setup(false);
    f.gpu_environment
        .io
        .0
        .lock()
        .unwrap()
        .device
        .automatic_power_off = false;
    let settings = crate::profiles::capture_current(&f.hal, &f.gpu, f.devices()).unwrap();
    let target = settings.gpu_power.as_ref().unwrap();
    assert!(!target.values.enabled);
    assert!(!target.values.automatic_power_off);
    assert!(settings.gpu.is_none());
    let decoded: ProfileSettings =
        serde_json::from_slice(&serde_json::to_vec(&settings).unwrap()).unwrap();
    assert!(decoded == settings);
    assert!(
        !f.gpu_power
            .read_runtime(&f.hal, &f.gpu)
            .unwrap()
            .can_restore
    );
    assert_eq!(f.gpu_environment.authorizations.load(Ordering::SeqCst), 0);
}
#[test]
fn unsupported_device_is_not_captured_or_fabricated() {
    let f = Fixture::new();
    let settings = crate::profiles::capture_current(&f.hal, &f.gpu, f.devices()).unwrap();
    assert!(settings.gpu_power.is_none());
    assert!(settings.gpu.is_some());
}
#[test]
fn strict_target_rejects_unknown_aliases_invalid_instances_and_conflicts() {
    let f = setup(true);
    let mut settings = wanted(&f, false);
    let mut encoded = serde_json::to_value(&settings).unwrap();
    encoded["gpuPower"]["enabled"] = false.into();
    assert!(serde_json::from_value::<ProfileSettings>(encoded).is_err());
    settings.gpu_power.as_mut().unwrap().id = "PCI\\VEN_10DE\\OTHER".into();
    assert!(crate::profiles::validate(&settings).is_err());
    settings = wanted(&f, false);
    settings.gpu = Some(crate::gpuctl::GpuClockSettings {
        core_offset_mhz: Some(60),
        mem_offset_mhz: None,
    });
    assert!(apply_settings(&f.hal, &f.gpu, f.devices(), &settings).is_err());
    settings.gpu = None;
    settings.fan_software_curves = Some(curves());
    assert!(crate::profiles::validate(&settings).is_err());
    assert!(f.gpu_environment.io.0.lock().unwrap().events.is_empty());
}
#[test]
fn unchanged_target_requires_no_authorization_or_initial_baseline() {
    let f = setup(true);
    success(&apply(&f, &wanted(&f, true)));
    assert_eq!(f.gpu_environment.authorizations.load(Ordering::SeqCst), 0);
    assert!(f.gpu_environment.io.0.lock().unwrap().events.is_empty());
    assert!(
        !f.gpu_power
            .read_runtime(&f.hal, &f.gpu)
            .unwrap()
            .can_restore
    );
}
#[test]
fn disabled_gpu_is_enabled_before_driver_offsets_and_other_targets() {
    let f = setup(false);
    let mut settings = wanted(&f, true);
    settings.pl1_w = Some(77.0);
    settings.gpu = Some(crate::gpuctl::GpuClockSettings {
        core_offset_mhz: Some(60),
        mem_offset_mhz: None,
    });
    let steps = apply(&f, &settings);
    success(&steps);
    assert_eq!(steps[0].step, "开启独显与策略");
    assert!(f.gpu_environment.io.device().enabled);
    assert_eq!(f.device.0.lock().unwrap().gpu_khz, 60_000);
    assert_eq!(f.gpu_environment.authorizations.load(Ordering::SeqCst), 1);
}
#[test]
fn unavailable_driver_after_enable_restores_off_and_writes_no_other_targets() {
    let mut f = setup(false);
    f.gpu = GpuController::new();
    let mut settings = wanted(&f, true);
    settings.pl1_w = Some(77.0);
    settings.gpu = Some(crate::gpuctl::GpuClockSettings {
        core_offset_mhz: Some(60),
        mem_offset_mhz: None,
    });
    let steps = apply(&f, &settings);
    assert!(steps.iter().any(|s| !s.ok && s.step == "档案开启后复核"));
    assert!(steps.iter().any(|s| s.ok && s.step == "恢复本次独显状态"));
    assert!(!f.gpu_environment.io.device().enabled);
    assert!(f.device.0.lock().unwrap().events.is_empty());
}
#[test]
fn unavailable_temperature_after_enable_restores_off_before_other_targets() {
    let f = setup(false);
    f.gpu_environment.io.0.lock().unwrap().temperature = None;
    let mut settings = wanted(&f, true);
    settings.pl1_w = Some(77.0);
    settings.fan_software_curves = Some(curves());
    let steps = apply(&f, &settings);
    assert!(
        steps
            .iter()
            .any(|s| !s.ok && s.message.contains("GPU 温度丢失"))
    );
    assert!(!f.gpu_environment.io.device().enabled);
    assert!(f.device.0.lock().unwrap().events.is_empty());
}
#[test]
fn successful_shutdown_and_single_actions_share_first_mixed_baseline() {
    let f = setup(true);
    f.gpu_environment
        .io
        .0
        .lock()
        .unwrap()
        .device
        .automatic_power_off = true;
    let before = f.gpu_environment.io.device();
    success(&apply(&f, &wanted(&f, false)));
    assert!(f.hal.gpu_suspended());
    let current = f.gpu_environment.io.device();
    f.gpu_power
        .apply_runtime(&f.hal, &f.gpu, &f.display.controller, &current, true)
        .unwrap();
    f.gpu_power
        .restore_runtime(&f.hal, &f.gpu, &f.display.controller)
        .unwrap();
    assert_eq!(f.gpu_environment.io.device(), before);
    assert_eq!(f.gpu_environment.authorizations.load(Ordering::SeqCst), 3);
}
#[test]
fn later_failure_restores_original_power_then_raw_offsets_in_same_authorization() {
    let f = setup(true);
    {
        let mut data = f.gpu_environment.io.0.lock().unwrap();
        data.device.automatic_power_off = true;
        let device = f.device.clone();
        data.node_hook = Some(Arc::new(move |enabled| {
            let mut state = device.0.lock().unwrap();
            state.events.push(format!("POWER:{enabled}"));
            state.gpu_khz = 0;
        }));
    }
    f.device.0.lock().unwrap().fail_gpu = true;
    let before = f.gpu_environment.io.device();
    let mut settings = wanted(&f, true);
    settings.pl1_w = Some(77.0);
    settings.gpu = Some(crate::gpuctl::GpuClockSettings {
        core_offset_mhz: Some(60),
        mem_offset_mhz: None,
    });
    let steps = apply(&f, &settings);
    assert!(steps.iter().any(|s| !s.ok && s.step == "GPU 偏移"));
    assert_eq!(f.gpu_environment.io.device(), before);
    let state = f.device.0.lock().unwrap();
    assert_eq!(state.gpu_khz, 135_000);
    let power = state
        .events
        .iter()
        .rposition(|s| s == "POWER:true")
        .unwrap();
    let offsets = state
        .events
        .iter()
        .rposition(|s| s == "GPU:135000")
        .unwrap();
    assert!(offsets > power);
    assert_eq!(f.gpu_environment.authorizations.load(Ordering::SeqCst), 1);
}
#[test]
fn active_output_rejection_is_before_cpu_or_authorization() {
    let f = setup(true);
    f.gpu_environment.io.0.lock().unwrap().device.can_disable = false;
    let mut settings = wanted(&f, false);
    settings.pl1_w = Some(77.0);
    assert!(apply_settings(&f.hal, &f.gpu, f.devices(), &settings).is_err());
    assert!(f.device.0.lock().unwrap().events.is_empty());
    assert_eq!(f.gpu_environment.authorizations.load(Ordering::SeqCst), 0);
}

#[test]
fn shutdown_recovery_uses_offsets_reread_after_authorization() {
    let f = setup(true);
    f.display.enable();
    let source = f.device.clone();
    f.gpu_environment.io.0.lock().unwrap().authorize_hook = Some(Box::new(move || {
        source.0.lock().unwrap().gpu_khz = 222_500;
    }));
    let device = f.device.clone();
    let display = f.display.io.clone();
    f.gpu_environment.io.0.lock().unwrap().node_hook = Some(Arc::new(move |enabled| {
        device.0.lock().unwrap().gpu_khz = 0;
        if !enabled {
            display.0.lock().unwrap().displays[0].current.frequency = 144;
        }
    }));
    let mut settings = wanted(&f, false);
    settings.display = f.display.controller.capture().unwrap();
    let steps = apply(&f, &settings);
    assert!(steps.iter().any(|s| s.step == "显示刷新率复核" && !s.ok));
    assert!(f.gpu_environment.io.device().enabled);
    assert_eq!(f.device.0.lock().unwrap().gpu_khz, 222_500);
}
