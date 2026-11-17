use super::*;
use crate::battery;
use fixtures::battery_fixture::policy;
use serde_json::json;
#[path = "battery_recovery_tests.rs"]
mod recovery;

fn charge(f: &Fixture, enabled: bool, start: u8, stop: u8) -> battery::ChargeTarget {
    let state = f.battery.io.state.lock().unwrap().charge.clone().unwrap();
    battery::ChargeTarget {
        values: battery::ChargeValues {
            enabled,
            start,
            stop,
        },
        start_values: state.start_values,
        stop_values: state.stop_values,
    }
}
fn energy(f: &Fixture, enabled: bool) -> battery::energy::Target {
    battery::energy::Target {
        capability: f
            .battery
            .io
            .state
            .lock()
            .unwrap()
            .capability
            .clone()
            .unwrap(),
        policy: policy(enabled),
    }
}
fn apply(f: &Fixture, settings: &ProfileSettings) -> Vec<ApplyStep> {
    apply_settings(&f.hal, &f.gpu, f.devices(), settings).unwrap()
}
fn set_energy(f: &Fixture, enabled: bool) {
    let profile = f.battery.controller.read_energy(&f.hal).unwrap().unwrap();
    f.battery
        .controller
        .apply_energy(
            &f.hal,
            &battery::energy::Request {
                expected_revision: profile.revision,
                policy: policy(enabled),
            },
        )
        .unwrap();
}
fn file(f: &Fixture) -> serde_json::Value {
    serde_json::from_slice(&std::fs::read(f.battery.path()).unwrap()).unwrap()
}

#[test]
fn capture_and_file_roundtrip_preserve_independent_charge_and_disabled_plan() {
    let f = Fixture::new();
    f.battery.enable();
    let settings = crate::profiles::capture_current(&f.hal, &f.gpu, f.devices()).unwrap();
    assert_eq!(
        settings.battery_charge.as_ref().unwrap().values,
        charge(&f, true, 60, 80).values
    );
    assert_eq!(
        settings.battery_energy.as_ref().unwrap(),
        &energy(&f, false)
    );
    let dir = tempfile::tempdir().unwrap();
    crate::profiles::save_in(dir.path(), settings.clone(), "电池").unwrap();
    assert!(
        crate::profiles::load_from(dir.path(), "电池")
            .unwrap()
            .settings
            == settings
    );
    assert!(f.battery.io.state.lock().unwrap().events.is_empty());
    assert!(!f.battery.path().exists());
}

#[test]
fn active_energy_capture_owns_charge_and_does_not_create_conflicting_manual_target() {
    let f = Fixture::new();
    f.battery.enable();
    set_energy(&f, true);
    let events = f.battery.io.state.lock().unwrap().events.len();
    let settings = crate::profiles::capture_current(&f.hal, &f.gpu, f.devices()).unwrap();
    assert!(settings.battery_charge.is_none());
    assert_eq!(settings.battery_energy.as_ref().unwrap(), &energy(&f, true));
    assert_eq!(f.battery.io.state.lock().unwrap().events.len(), events);
}

#[test]
fn charge_profile_and_single_restore_share_the_first_baseline() {
    let f = Fixture::new();
    f.battery.enable();
    let initial = f.battery.controller.read(&f.hal).unwrap().revision.unwrap();
    f.battery
        .controller
        .apply(
            &f.hal,
            &battery::ChargeRequest {
                expected_revision: initial,
                values: charge(&f, true, 70, 90).values,
            },
        )
        .unwrap();
    let settings = ProfileSettings {
        battery_charge: Some(charge(&f, true, 80, 100)),
        ..Default::default()
    };
    assert!(apply(&f, &settings).iter().all(|step| step.ok));
    let revision = f.battery.controller.read(&f.hal).unwrap().revision.unwrap();
    f.battery.controller.restore(&f.hal, &revision).unwrap();
    assert_eq!(
        f.battery
            .io
            .state
            .lock()
            .unwrap()
            .charge
            .as_ref()
            .unwrap()
            .current,
        battery::ChargeValues {
            enabled: true,
            start: 60,
            stop: 80
        }
    );
}

#[test]
fn enabling_energy_profile_keeps_persistent_first_charge_and_stop_recovers_it() {
    let f = Fixture::new();
    f.battery.enable();
    let settings = ProfileSettings {
        battery_energy: Some(energy(&f, true)),
        ..Default::default()
    };
    assert!(apply(&f, &settings).iter().all(|step| step.ok));
    assert_eq!(
        file(&f)["stopCharge"]["current"],
        json!({"enabled":true,"start":60,"stop":80})
    );
    assert_eq!(
        f.battery
            .io
            .state
            .lock()
            .unwrap()
            .charge
            .as_ref()
            .unwrap()
            .current
            .start,
        95
    );
    set_energy(&f, false);
    assert_eq!(
        f.battery
            .io
            .state
            .lock()
            .unwrap()
            .charge
            .as_ref()
            .unwrap()
            .current
            .start,
        60
    );
    assert_eq!(file(&f)["baseline"]["charge"]["current"]["stop"], 80);
}

#[test]
fn stopping_energy_precedes_manual_charge_and_manual_restore_uses_projected_stop_state() {
    let f = Fixture::new();
    f.battery.enable();
    set_energy(&f, true);
    f.battery.io.state.lock().unwrap().events.clear();
    let settings = ProfileSettings {
        battery_energy: Some(energy(&f, false)),
        battery_charge: Some(charge(&f, true, 70, 90)),
        ..Default::default()
    };
    assert!(apply(&f, &settings).iter().all(|step| step.ok));
    {
        let state = f.battery.io.state.lock().unwrap();
        assert_eq!(
            &state.events[state.events.len() - 2..],
            ["CHARGE:true:60:80", "CHARGE:true:70:90"]
        );
    }
    let revision = f.battery.controller.read(&f.hal).unwrap().revision.unwrap();
    f.battery.controller.restore(&f.hal, &revision).unwrap();
    assert_eq!(
        f.battery
            .io
            .state
            .lock()
            .unwrap()
            .charge
            .as_ref()
            .unwrap()
            .current
            .start,
        60
    );
}

#[test]
fn later_gpu_failure_restores_enabled_plan_charge_and_exact_previous_record() {
    let f = Fixture::new();
    f.battery.enable();
    set_energy(&f, true);
    let previous = std::fs::read(f.battery.path()).unwrap();
    f.device.0.lock().unwrap().fail_gpu = true;
    let settings = ProfileSettings {
        battery_energy: Some(energy(&f, false)),
        battery_charge: Some(charge(&f, true, 70, 90)),
        gpu: Some(crate::gpuctl::GpuClockSettings {
            core_offset_mhz: Some(60),
            mem_offset_mhz: None,
        }),
        ..Default::default()
    };
    let steps = apply(&f, &settings);
    assert!(
        steps
            .iter()
            .any(|step| step.step == "恢复电池配置" && step.ok)
    );
    assert_eq!(std::fs::read(f.battery.path()).unwrap(), previous);
    assert_eq!(
        f.battery
            .io
            .state
            .lock()
            .unwrap()
            .charge
            .as_ref()
            .unwrap()
            .current,
        battery::ChargeValues {
            enabled: true,
            start: 95,
            stop: 100
        }
    );
}

#[test]
fn later_failure_after_first_energy_profile_keeps_original_baseline_and_no_pending_plan() {
    let f = Fixture::new();
    f.battery.enable();
    f.device.0.lock().unwrap().fail_gpu = true;
    let settings = ProfileSettings {
        battery_energy: Some(energy(&f, true)),
        gpu: Some(crate::gpuctl::GpuClockSettings {
            core_offset_mhz: Some(60),
            mem_offset_mhz: None,
        }),
        ..Default::default()
    };
    let steps = apply(&f, &settings);
    assert!(
        steps
            .iter()
            .any(|step| step.step == "恢复电池配置" && step.ok)
    );
    assert_eq!(file(&f)["pending"], serde_json::Value::Null);
    assert_eq!(file(&f)["policy"]["enabled"], false);
    assert_eq!(
        f.battery
            .io
            .state
            .lock()
            .unwrap()
            .charge
            .as_ref()
            .unwrap()
            .current
            .start,
        60
    );
}

#[test]
fn invalid_battery_targets_and_runtime_conditions_prevent_every_profile_write() {
    for case in 0..7 {
        let f = Fixture::new();
        f.battery.enable();
        let mut settings = ProfileSettings {
            battery_charge: Some(charge(&f, true, 70, 90)),
            runtime_cpu: Some(target()),
            ..Default::default()
        };
        match case {
            0 => settings.battery_charge.as_mut().unwrap().start_values = vec![70],
            1 => f.battery.io.state.lock().unwrap().present = None,
            2 => f.battery.io.state.lock().unwrap().present = Some(false),
            3 => settings.battery_energy = Some(energy(&f, true)),
            4 => f.battery.io.state.lock().unwrap().original = Some(policy(true)),
            5 => {
                let mut e = energy(&f, false);
                e.capability.factory_defaulted = true;
                settings.battery_energy = Some(e);
            }
            _ => {
                let mut e = energy(&f, false);
                e.policy.slots[1] = e.policy.slots[0].clone();
                settings.battery_energy = Some(e);
            }
        }
        assert!(
            apply_settings(&f.hal, &f.gpu, f.devices(), &settings).is_err(),
            "case {case}"
        );
        assert!(
            f.battery.io.state.lock().unwrap().events.is_empty(),
            "case {case}"
        );
        assert!(f.device.0.lock().unwrap().events.is_empty(), "case {case}");
    }
}

#[test]
fn raw_disabled_zero_charge_profile_replays_exactly_after_a_later_manual_setting() {
    let f = Fixture::new();
    f.battery.enable();
    f.battery
        .io
        .state
        .lock()
        .unwrap()
        .charge
        .as_mut()
        .unwrap()
        .current = battery::ChargeValues {
        enabled: false,
        start: 0,
        stop: 0,
    };
    let captured = charge(&f, false, 0, 0);
    let revision = f.battery.controller.read(&f.hal).unwrap().revision.unwrap();
    f.battery
        .controller
        .apply(
            &f.hal,
            &battery::ChargeRequest {
                expected_revision: revision,
                values: charge(&f, true, 70, 90).values,
            },
        )
        .unwrap();
    assert!(
        apply(
            &f,
            &ProfileSettings {
                battery_charge: Some(captured),
                ..Default::default()
            }
        )
        .iter()
        .all(|step| step.ok)
    );
    assert_eq!(
        f.battery
            .io
            .state
            .lock()
            .unwrap()
            .charge
            .as_ref()
            .unwrap()
            .current,
        battery::ChargeValues {
            enabled: false,
            start: 0,
            stop: 0
        }
    );
}

#[test]
fn incomplete_unknown_or_conflicting_battery_json_never_becomes_an_accepted_profile() {
    let f = Fixture::new();
    f.battery.enable();
    let settings = ProfileSettings {
        battery_charge: Some(charge(&f, true, 70, 90)),
        ..Default::default()
    };
    let mut bad = serde_json::to_value(&settings).unwrap();
    bad["batteryCharge"]["calibrate"] = json!(true);
    assert!(serde_json::from_value::<ProfileSettings>(bad).is_err());
    let mut bad = settings.clone();
    bad.battery_charge.as_mut().unwrap().start_values = vec![70, 70];
    assert!(crate::profiles::validate(&bad).is_err());
    let mut bad = settings;
    bad.battery_energy = Some(energy(&f, true));
    assert!(crate::profiles::validate(&bad).is_err());
}
