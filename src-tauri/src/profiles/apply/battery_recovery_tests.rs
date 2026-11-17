use super::*;

#[test]
fn absent_original_plan_is_not_captured_as_empty_and_later_failure_releases_only_application_slots()
{
    let f = Fixture::new();
    f.battery.enable();
    f.battery.io.state.lock().unwrap().original = None;
    let captured = crate::profiles::capture_current(&f.hal, &f.gpu, f.devices()).unwrap();
    assert!(captured.battery_energy.is_none());
    assert!(captured.battery_charge.is_some());
    f.device.0.lock().unwrap().fail_gpu = true;
    let settings = ProfileSettings {
        battery_energy: Some(energy(&f, true)),
        gpu: Some(crate::gpuctl::GpuClockSettings {
            core_offset_mhz: Some(60),
            mem_offset_mhz: None,
        }),
        ..Default::default()
    };
    assert!(
        apply(&f, &settings)
            .iter()
            .any(|step| step.step == "恢复电池配置" && step.ok)
    );
    assert!(file(&f)["policy"].is_null());
    assert!(file(&f)["baseline"]["policy"].is_null());
    let state = f.battery.io.state.lock().unwrap();
    assert!(
        state
            .commands
            .ends_with(&[0x05000000, 0x03000000, 0x04000000])
    );
    assert_eq!(state.charge.as_ref().unwrap().current.stop, 80);
}

#[test]
fn single_charge_waits_for_profile_scope_then_rejects_its_old_revision() {
    let f = Fixture::new();
    f.battery.enable();
    let revision = f.battery.controller.read(&f.hal).unwrap().revision.unwrap();
    let manual = battery::ChargeRequest {
        expected_revision: revision,
        values: charge(&f, true, 80, 100).values,
    };
    let profile_target = charge(&f, true, 70, 90);
    std::thread::scope(|threads| {
        let mut owner = f.battery.controller.scope().unwrap();
        let (started_tx, started) = std::sync::mpsc::channel();
        let (done_tx, done) = std::sync::mpsc::channel();
        let shared = &f;
        let writer = threads.spawn(move || {
            started_tx.send(()).unwrap();
            done_tx
                .send(shared.battery.controller.apply(&shared.hal, &manual))
                .unwrap();
        });
        started.recv().unwrap();
        assert!(
            done.recv_timeout(std::time::Duration::from_millis(30))
                .is_err()
        );
        let gate = f.hal.begin_write().unwrap();
        let plan = owner.prepare(&f.hal, Some(&profile_target), None).unwrap();
        owner
            .execute(&f.hal, &plan, &mut battery::Change::default())
            .unwrap();
        drop(gate);
        drop(owner);
        assert!(
            done.recv_timeout(std::time::Duration::from_secs(2))
                .unwrap()
                .is_err()
        );
        writer.join().unwrap();
    });
    assert_eq!(f.battery.io.state.lock().unwrap().charges.len(), 1);
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
fn native_battery_controller_without_test_io_cannot_fall_back_to_system_or_oem_reads() {
    let f = Fixture::new();
    let dir = tempfile::tempdir().unwrap();
    let controller = battery::Controller::new(dir.path().join("energy-policy.json"));
    assert!(
        controller
            .read(&f.hal)
            .err()
            .unwrap()
            .to_string()
            .contains("未注入充电IO")
    );
    assert!(
        controller
            .read_energy(&f.hal)
            .err()
            .unwrap()
            .to_string()
            .contains("未注入充电IO")
    );
    assert!(f.device.0.lock().unwrap().events.is_empty());
}

#[test]
fn original_enabled_schedule_blocks_single_charge_even_without_application_record() {
    let f = Fixture::new();
    f.battery.enable();
    f.battery.io.state.lock().unwrap().original = Some(policy(true));
    let profile = f.battery.controller.read(&f.hal).unwrap();
    assert!(profile.charge_lock_reason.unwrap().contains("节能计划"));
    let request = battery::ChargeRequest {
        expected_revision: profile.revision.unwrap(),
        values: charge(&f, true, 70, 90).values,
    };
    assert!(f.battery.controller.apply(&f.hal, &request).is_err());
    assert!(f.battery.io.state.lock().unwrap().events.is_empty());
    assert!(!f.battery.path().exists());
}

#[test]
fn energy_prepare_binds_the_previous_file_and_foreign_changes_prevent_any_mutation() {
    let f = Fixture::new();
    f.battery.enable();
    set_energy(&f, false);
    f.battery.io.state.lock().unwrap().events.clear();
    let wanted = energy(&f, true);
    let mut scope = f.battery.controller.scope().unwrap();
    let _gate = f.hal.begin_write().unwrap();
    let plan = scope.prepare(&f.hal, None, Some(&wanted)).unwrap();
    let mut changed = file(&f);
    changed["lastError"] = json!("外部记录");
    let bytes = serde_json::to_vec_pretty(&changed).unwrap();
    std::fs::write(f.battery.path(), &bytes).unwrap();
    let mut change = battery::Change::default();
    assert!(scope.execute(&f.hal, &plan, &mut change).is_err());
    assert!(!change.dirty());
    assert!(f.battery.io.state.lock().unwrap().events.is_empty());
    assert_eq!(std::fs::read(f.battery.path()).unwrap(), bytes);
}

#[test]
fn foreign_record_after_first_energy_output_stops_later_commands_and_old_compensation() {
    let f = Fixture::new();
    f.battery.enable();
    let path = f.battery.path();
    *f.battery.io.after_command.lock().unwrap() = Some(Box::new(move || {
        let mut record: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        record["lastError"] = json!("外部拥有的恢复记录");
        std::fs::write(path, serde_json::to_vec_pretty(&record).unwrap()).unwrap();
    }));
    let settings = ProfileSettings {
        battery_energy: Some(energy(&f, true)),
        ..Default::default()
    };
    let steps = apply(&f, &settings);
    assert!(steps.iter().any(|step| step.step == "电池配置" && !step.ok));
    assert!(
        steps
            .iter()
            .any(|step| step.step == "恢复电池配置" && !step.ok)
    );
    assert_eq!(f.battery.io.state.lock().unwrap().commands.len(), 1);
    assert!(f.battery.io.state.lock().unwrap().charges.is_empty());
    assert_eq!(file(&f)["lastError"], "外部拥有的恢复记录");
    assert!(!file(&f)["pending"].is_null());
}

#[test]
fn energy_partial_failure_recovers_locally_without_a_second_profile_replay() {
    let f = Fixture::new();
    f.battery.enable();
    f.battery.io.state.lock().unwrap().fail_commands = vec![3];
    let settings = ProfileSettings {
        battery_energy: Some(energy(&f, true)),
        ..Default::default()
    };
    let steps = apply(&f, &settings);
    assert!(steps.iter().any(|step| step.step == "电池配置" && !step.ok));
    assert!(!steps.iter().any(|step| step.step == "恢复电池配置"));
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
    assert!(file(&f)["pending"].is_null());
}

#[test]
fn failed_local_energy_recovery_is_retried_by_outer_profile_then_explicit_restore() {
    let f = Fixture::new();
    f.battery.enable();
    f.battery.io.state.lock().unwrap().fail_commands = vec![3, 4];
    let settings = ProfileSettings {
        battery_energy: Some(energy(&f, true)),
        ..Default::default()
    };
    let steps = apply(&f, &settings);
    assert!(
        steps
            .iter()
            .any(|step| step.step == "恢复电池配置" && step.ok)
    );
    assert!(file(&f)["pending"].is_null());
    let profile = f.battery.controller.read_energy(&f.hal).unwrap().unwrap();
    f.battery
        .controller
        .restore_energy(&f.hal, &profile.revision)
        .unwrap();
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
            .stop,
        80
    );
}

#[test]
fn partial_charge_failure_retains_shared_initial_snapshot_and_retry() {
    let f = Fixture::new();
    f.battery.enable();
    f.battery.io.state.lock().unwrap().fail_charges = vec![1, 2];
    let settings = ProfileSettings {
        battery_charge: Some(charge(&f, true, 70, 90)),
        ..Default::default()
    };
    let steps = apply(&f, &settings);
    assert!(
        steps
            .iter()
            .any(|step| step.step == "恢复电池配置" && step.ok)
    );
    let profile = f.battery.controller.read(&f.hal).unwrap();
    assert!(profile.can_restore);
    f.battery
        .controller
        .restore(&f.hal, &profile.revision.unwrap())
        .unwrap();
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
            .stop,
        80
    );
}

#[test]
fn charge_and_energy_current_state_changes_after_preparation_fail_before_output() {
    for use_energy in [false, true] {
        let f = Fixture::new();
        f.battery.enable();
        let charge = charge(&f, true, 70, 90);
        let energy = energy(&f, true);
        let mut scope = f.battery.controller.scope().unwrap();
        let _gate = f.hal.begin_write().unwrap();
        let plan = scope
            .prepare(
                &f.hal,
                (!use_energy).then_some(&charge),
                use_energy.then_some(&energy),
            )
            .unwrap();
        f.battery
            .io
            .state
            .lock()
            .unwrap()
            .charge
            .as_mut()
            .unwrap()
            .current
            .start = 50;
        let mut change = battery::Change::default();
        assert!(scope.execute(&f.hal, &plan, &mut change).is_err());
        assert!(!change.dirty());
        assert!(f.battery.io.state.lock().unwrap().events.is_empty());
        assert!(!f.battery.path().exists());
    }
}

#[test]
fn calibration_pending_or_corrupt_record_blocks_capture_and_all_profile_targets() {
    for corrupt in [false, true] {
        let f = Fixture::new();
        f.battery.enable();
        let path = f.battery.dir.path().join("battery-calibration.json");
        if corrupt {
            std::fs::write(path, b"{broken").unwrap();
        } else {
            use battery::calibration::{
                Baseline,
                store::{FileStore, Record, Store},
            };
            let baseline: Baseline = serde_json::from_value(
                json!({"stopThreshold":80,"powerScheme":"381b4222-f694-41f0-9685-ff5bb260df2e",
                "charge":f.battery.io.state.lock().unwrap().charge,"energy":null}),
            )
            .unwrap();
            FileStore(path)
                .save(
                    &Record::prepared("1234567890abcdef1234567890abcdef".into(), baseline).unwrap(),
                )
                .unwrap();
        }
        let settings = ProfileSettings {
            battery_charge: Some(charge(&f, true, 70, 90)),
            runtime_cpu: Some(target()),
            ..Default::default()
        };
        assert!(apply_settings(&f.hal, &f.gpu, f.devices(), &settings).is_err());
        assert!(crate::profiles::capture_current(&f.hal, &f.gpu, f.devices()).is_err());
        assert!(f.battery.io.state.lock().unwrap().events.is_empty());
        assert!(f.device.0.lock().unwrap().events.is_empty());
    }
}
