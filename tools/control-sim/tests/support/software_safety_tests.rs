use super::*;

#[test]
fn software_process_restored_hardware_is_not_rewritten_when_only_journal_commit_failed() {
    use std::os::windows::fs::OpenOptionsExt;
    let f = Fixture::new();
    let hal = f.hal();
    {
        let _gate = hal.begin_write().unwrap();
        fanctl::software::apply(hal, &curves()).unwrap();
    }
    let path = f.dir.join(hal::SOFTWARE_FAN_RECORD_NAME);
    let held = fs::OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(path)
        .unwrap();
    *hal.software_progress.lock().unwrap() = Some(Instant::now() - Duration::from_secs(4));
    assert!(hal.pulse_fan_guard().is_err());
    f.wait_file("restore.log");
    f.wait_guard_exit();
    assert!(hal.supervise_fan_guard().is_err());
    assert!(hal.safety_status().contains("已恢复"));
    assert!(hal.safety_status().contains("未提交"));
    let count = DEVICE.lock().unwrap().writes.len();
    for _ in 0..3 {
        hal.supervise_fan_guard().unwrap();
    }
    assert_eq!(DEVICE.lock().unwrap().writes.len(), count);
    assert!(hal.ensure_software_fan_restart_allowed().is_err());
    drop(held);
    {
        let _gate = hal.begin_write().unwrap();
        fanctl::software::apply_confirmed(hal, &curves()).unwrap();
    }
    fanctl::release_on_exit(hal).unwrap();
}

#[test]
fn software_process_confirmation_commit_failure_releases_the_curve_and_preserves_restart_block() {
    let f = Fixture::new();
    let hal = f.hal();
    DEVICE.lock().unwrap().hold_on_duty = true;
    {
        let _gate = hal.begin_write().unwrap();
        let error = fanctl::software::apply_confirmed(hal, &curves()).unwrap_err();
        assert!(error.to_string().contains("保存软件风扇恢复记录失败"));
    }
    assert!(DEVICE.lock().unwrap().journal_hold.is_some());
    assert!(fanctl::software::read(hal).unwrap().is_none());
    assert!(!hal.fan_guard_armed().unwrap());
    assert!(hal.ensure_software_fan_restart_allowed().is_err());
    DEVICE.lock().unwrap().journal_hold.take();
    {
        let _gate = hal.begin_write().unwrap();
        fanctl::software::apply_confirmed(hal, &curves()).unwrap();
    }
    hal.ensure_software_fan_restart_allowed().unwrap();
    fanctl::release_on_exit(hal).unwrap();
}

#[test]
fn software_process_records_the_first_output_and_a_normal_exit_clears_only_running_state() {
    let f = Fixture::new();
    let hal = f.hal();
    {
        let _gate = hal.begin_write().unwrap();
        fanctl::software::apply(hal, &curves()).unwrap();
    }
    assert_eq!(DEVICE.lock().unwrap().unjournaled, 0);
    let path = f.dir.join(hal::SOFTWARE_FAN_RECORD_NAME);
    let record: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(record["active"], true);
    fanctl::release_on_exit(hal).unwrap();
    assert!(fanctl::software::read(hal).unwrap().is_none());
    assert!(!hal.fan_guard_armed().unwrap());
    let count = DEVICE.lock().unwrap().writes.len();
    {
        let _gate = hal.begin_write().unwrap();
        assert!(fanctl::software::apply(hal, &curves()).is_err());
        assert!(fanctl::software::apply_confirmed(hal, &curves()).is_err());
    }
    assert_eq!(DEVICE.lock().unwrap().writes.len(), count);
    let mut next = hal::Hal::test_instance(
        hal::oem_dchu::Dchu::test_io(buffer, read, set, write).with_test_exchange(exchange),
        None,
    );
    next.bind_software_fan_record(path);
    next.ensure_software_fan_restart_allowed().unwrap();
}

#[test]
fn software_process_journal_failure_rejects_the_first_duty_and_manual_retry_can_recover() {
    use std::os::windows::fs::OpenOptionsExt;
    let f = Fixture::new();
    let hal = f.hal();
    hal.record_software_fan_started(false).unwrap();
    hal.record_software_fan_stopped().unwrap();
    let path = f.dir.join(hal::SOFTWARE_FAN_RECORD_NAME);
    let held = fs::OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(&path)
        .unwrap();
    {
        let _gate = hal.begin_write().unwrap();
        assert!(fanctl::software::apply(hal, &curves()).is_err());
    }
    assert!(
        DEVICE
            .lock()
            .unwrap()
            .writes
            .iter()
            .all(|(cmd, bytes)| *cmd != 4 || bytes.get(6) != Some(&0xc1))
    );
    assert!(hal.ensure_software_fan_restart_allowed().is_err());
    drop(held);
    {
        let _gate = hal.begin_write().unwrap();
        fanctl::software::apply_confirmed(hal, &curves()).unwrap();
    }
    hal.ensure_software_fan_restart_allowed().unwrap();
    fanctl::release_on_exit(hal).unwrap();
    assert_eq!(DEVICE.lock().unwrap().unjournaled, 0);
}

#[test]
fn software_process_fuse_recovers_and_never_resumes_without_confirmation() {
    let f = Fixture::new();
    let hal = f.hal();
    {
        let _write = hal.begin_write().unwrap();
        fanctl::software::apply(hal, &curves()).unwrap();
    }
    DEVICE.lock().unwrap().cpu = 95;
    assert!(
        fanctl::software::tick(hal)
            .unwrap_err()
            .to_string()
            .contains("熔断")
    );
    assert!(fanctl::software::read(hal).unwrap().is_none());
    DEVICE.lock().unwrap().cpu = 50;
    let count = DEVICE.lock().unwrap().writes.len();
    fanctl::software::tick(hal).unwrap();
    assert_eq!(DEVICE.lock().unwrap().writes.len(), count);
    assert!(hal.ensure_software_fan_restart_allowed().is_err());
    {
        let _write = hal.begin_write().unwrap();
        DEVICE.lock().unwrap().fail_duty = true;
        assert!(fanctl::software::apply_confirmed(hal, &curves()).is_err());
        assert!(hal.ensure_software_fan_restart_allowed().is_err());
        fanctl::software::apply_confirmed(hal, &curves()).unwrap();
        hal.ensure_software_fan_restart_allowed().unwrap();
        fanctl::software::stop(hal).unwrap();
        hal.disarm_fan_guard().unwrap();
    }
}

#[test]
fn software_process_faulted_checkpoint_restores_fields_without_restarting_the_curve() {
    let f = Fixture::new();
    let hal = f.hal();
    let checkpoint = {
        let _write = hal.begin_write().unwrap();
        fanctl::software::apply(hal, &curves()).unwrap();
        fanctl::checkpoint(hal, false).unwrap()
    };
    DEVICE.lock().unwrap().cpu = 95;
    assert!(fanctl::software::tick(hal).is_err());
    DEVICE.lock().unwrap().cpu = 50;
    let count = DEVICE.lock().unwrap().writes.len();
    {
        let _write = hal.begin_write().unwrap();
        let error = fanctl::restore_checkpoint(hal, &checkpoint).unwrap_err();
        assert!(error.to_string().contains("因故障停止"));
    }
    assert!(fanctl::software::read(hal).unwrap().is_none());
    assert!(hal.ensure_software_fan_restart_allowed().is_err());
    assert!(
        DEVICE.lock().unwrap().writes[count..]
            .iter()
            .all(|(cmd, _)| *cmd != 4)
    );
    assert!(!hal.fan_guard_armed().unwrap());
}
