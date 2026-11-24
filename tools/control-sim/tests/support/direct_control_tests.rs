use super::*;
use hal::firmware::DirectControl;

fn running_record(f: &Fixture) -> bool {
    let bytes = fs::read(f.dir.join(hal::SOFTWARE_FAN_RECORD_NAME)).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    value["active"].as_bool().unwrap()
}

#[test]
fn reviewed_dsdt_without_control_readback_rejects_preflight_and_takeover_before_any_write() {
    let mut f = Fixture::new();
    f.hal.as_mut().unwrap().test_fan_control = None;
    let hal = f.hal();
    assert!(hal.reviewed_dsdt);
    let _write = hal.begin_write().unwrap();
    for result in [
        fanctl::software::validate_configuration(hal, &curves()),
        fanctl::software::apply(hal, &curves()),
        fanctl::software::apply_manual(hal, &curves()),
    ] {
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("无法核实逐通道风扇控制权")
        );
    }
    assert!(DEVICE.lock().unwrap().writes.is_empty());
    assert!(f.guards.lock().unwrap().is_empty());
    assert!(!f.dir.join(hal::SOFTWARE_FAN_RECORD_NAME).exists());
}

#[test]
fn an_unclean_record_without_a_reader_keeps_the_exit_release_unconfirmed() {
    let mut f = Fixture::new();
    let hal = f.hal.as_mut().unwrap();
    let path = f.dir.join(hal::SOFTWARE_FAN_RECORD_NAME);
    hal.record_software_fan_started(false).unwrap();
    hal.bind_software_fan_record(path);
    hal.test_fan_control = None;
    assert!(hal.safety_status().contains("风扇归还未证实"));
    assert!(fanctl::release_on_exit(hal).is_err());
    assert!(fanctl::software::active(hal).unwrap());
    assert!(!hal.fan_recovery_pending());
    assert!(running_record(&f));
    assert_eq!(
        DEVICE.lock().unwrap().writes,
        [(0x69, 3u32.to_le_bytes().to_vec())]
    );
}

#[test]
fn a_c1_echo_without_confirmed_control_does_not_advance_the_session() {
    let f = Fixture::new();
    DEVICE.lock().unwrap().ignore_duty = true;
    let hal = f.hal();
    let _write = hal.begin_write().unwrap();
    let error = fanctl::software::apply(hal, &curves()).unwrap_err();
    assert!(error.to_string().contains("目标占空比回读不一致"));
    assert!(fanctl::software::read(hal).unwrap().is_none());
    assert!(hal.software_progress.lock().unwrap().is_none());
    assert!(!running_record(&f));
    assert!(!hal.fan_guard_armed().unwrap());
    assert!(hal.ensure_software_fan_restart_allowed().is_err());
}

#[test]
fn a_running_curve_stops_before_another_output_when_turbo_or_dust_conflicts() {
    for (field, flags, expected) in [(8, 1, "Turbo"), (10, 0x80, "除尘")] {
        let mut f = Fixture::new();
        configure_options(&mut f, 2, 0);
        let hal = f.hal();
        {
            let _write = hal.begin_write().unwrap();
            fanctl::software::apply(hal, &curves()).unwrap();
        }
        {
            let mut device = DEVICE.lock().unwrap();
            let page = if field == 8 { 4 } else { 7 };
            device.pages[page][field] = flags;
        }
        let error = fanctl::software::tick(hal).unwrap_err();
        assert!(error.to_string().contains(expected));
        assert!(!fanctl::software::active(hal).unwrap());
        let command_count = DEVICE
            .lock()
            .unwrap()
            .writes
            .iter()
            .filter(|(code, _)| *code == 4)
            .count();
        assert_eq!(command_count, 1);
    }
}

#[test]
fn a_release_echo_with_a_manual_fan_keeps_the_record_and_recovery_pending_on_exit() {
    let f = Fixture::new();
    let hal = f.hal();
    {
        let _write = hal.begin_write().unwrap();
        fanctl::software::apply(hal, &curves()).unwrap();
    }
    DEVICE.lock().unwrap().ignore_release = true;
    let error = fanctl::release_on_exit(hal).unwrap_err();
    assert!(format!("{error:#}").contains("EC 仍有风扇通道处于软件控制"));
    assert!(hal.fan_guard_armed().unwrap());
    assert!(hal.fan_recovery_pending());
    assert!(running_record(&f));
    assert!(fanctl::software::active(hal).unwrap());
    assert!(fanctl::software::read(hal).unwrap().is_none());
    assert!(fanctl::software::read_for_handoff(hal).is_err());
    f.wait_guard_exit();
    assert!(hal.supervise_fan_guard().is_err());
    assert!(matches!(
        DEVICE.lock().unwrap().control[0],
        DirectControl::Manual(_)
    ));
    assert!(running_record(&f));

    DEVICE.lock().unwrap().ignore_release = false;
    hal.supervise_fan_guard().unwrap();
    assert_eq!(DEVICE.lock().unwrap().control, [DirectControl::Firmware; 2]);
    assert!(!running_record(&f));
    assert!(!fanctl::software::active(hal).unwrap());
}

#[test]
fn a_blocked_release_still_expires_the_guard_progress_deadline() {
    use std::sync::mpsc;

    let f = Fixture::new();
    let hal = f.hal();
    {
        let _write = hal.begin_write().unwrap();
        fanctl::software::apply(hal, &curves()).unwrap();
    }
    let (signal, entered) = mpsc::sync_channel(1);
    let (finish, resume) = mpsc::channel();
    DEVICE.lock().unwrap().release_hold = Some((signal, resume));
    std::thread::scope(|threads| {
        let release = threads.spawn(|| fanctl::release_on_exit(hal));
        entered.recv_timeout(Duration::from_secs(4)).unwrap();
        {
            let mut progress = hal.software_progress.lock().unwrap();
            assert!(progress.is_some());
            *progress = Some(Instant::now() - Duration::from_secs(4));
        }
        assert!(hal.pulse_fan_guard().is_err());
        f.wait_file("restore.log");
        finish.send(()).unwrap();
        assert!(release.join().unwrap().is_err());
    });
    f.wait_guard_exit();
    assert!(running_record(&f));
    hal.supervise_fan_guard().unwrap();
    assert!(!running_record(&f));
}
