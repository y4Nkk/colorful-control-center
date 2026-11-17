use super::*;

#[test]
fn software_process_curve_to_firmware_full_speed_and_back_uses_one_guard() {
    let f = Fixture::new();
    let hal = f.hal();
    let _gate = hal.begin_write().unwrap();
    fanctl::software::apply(hal, &curves()).unwrap();
    fanctl::set_mode(hal, 1).unwrap();
    assert!(fanctl::software::read(hal).unwrap().is_none());
    assert_eq!(DEVICE.lock().unwrap().pages[4][5], 1);
    fs::write(f.dir.join("temperature"), "99").unwrap();
    DEVICE.lock().unwrap().cpu = 99;
    let outputs = DEVICE.lock().unwrap().writes.len();
    assert!(
        fanctl::set_mode(hal, 6)
            .unwrap_err()
            .to_string()
            .contains("保留当前全速散热")
    );
    assert_eq!(DEVICE.lock().unwrap().writes.len(), outputs);
    assert_eq!(DEVICE.lock().unwrap().pages[4][5], 1);
    fanctl::set_offset(hal, 30).unwrap();
    assert_eq!(DEVICE.lock().unwrap().pages[4][5], 1);
    fs::write(f.dir.join("temperature"), "50").unwrap();
    DEVICE.lock().unwrap().cpu = 50;
    fanctl::software::apply(hal, &curves()).unwrap();
    assert!(fanctl::software::read(hal).unwrap().is_some());
    assert_eq!(f.guards.lock().unwrap().len(), 1);
    assert!(!f.dir.join("restore.log").exists());
    fanctl::set_mode(hal, 0).unwrap();
}

#[test]
fn software_process_hot_full_speed_checkpoint_and_initial_restore_share_policy() {
    let f = Fixture::new();
    let hal = f.hal();
    let _gate = hal.begin_write().unwrap();
    DEVICE.lock().unwrap().pages[4][5] = 1;
    fs::write(f.dir.join("mode"), "1").unwrap();
    let checkpoint = fanctl::checkpoint(hal, false).unwrap();
    fanctl::set_offset(hal, 20).unwrap();
    fs::write(f.dir.join("temperature"), "99").unwrap();
    DEVICE.lock().unwrap().cpu = 99;
    fanctl::restore_checkpoint(hal, &checkpoint).unwrap();
    assert_eq!(DEVICE.lock().unwrap().pages[4][7], 0);
    fanctl::restore_initial(hal).unwrap();
    assert_eq!(DEVICE.lock().unwrap().pages[4][5], 1);
    assert!(!f.dir.join("restore.log").exists());
    fanctl::set_mode(hal, 0).unwrap();
}

impl Fixture {
    pub(super) fn wait_guard_exit(&self) {
        let directories = self.guards.lock().unwrap();
        for directory in directories.iter() {
            let deadline = Instant::now() + Duration::from_secs(8);
            while !directory.join("child.exit").exists() {
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(10));
            }
            let pid = fs::read_to_string(directory.join("child.pid"))
                .unwrap()
                .parse()
                .unwrap();
            while process::alive(pid) {
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }
}

#[test]
fn software_process_old_guard_finishes_recovery_before_another_full_speed_target_is_allowed() {
    let f = Fixture::new();
    let hal = f.hal();
    {
        let _gate = hal.begin_write().unwrap();
        fanctl::software::apply(hal, &curves()).unwrap();
    }
    fs::write(f.dir.join("hold-recovery"), "1").unwrap();
    hal.request_fan_recovery().unwrap();
    f.wait_file("restore.log");
    let before = DEVICE.lock().unwrap().writes.len();
    assert!(
        hal.supervise_fan_guard()
            .unwrap_err()
            .to_string()
            .contains("仍在恢复")
    );
    {
        let _gate = hal.begin_write().unwrap();
        assert!(fanctl::set_mode(hal, 1).is_err());
    }
    assert_eq!(DEVICE.lock().unwrap().writes.len(), before);
    assert_eq!(f.guards.lock().unwrap().len(), 1);
    fs::write(f.dir.join("release-recovery"), "1").unwrap();
    f.wait_guard_exit();
    hal.supervise_fan_guard().unwrap();
    {
        let _gate = hal.begin_write().unwrap();
        fanctl::set_mode(hal, 1).unwrap();
        assert_eq!(DEVICE.lock().unwrap().pages[4][5], 1);
        fanctl::set_mode(hal, 0).unwrap();
    }
    assert_eq!(f.guards.lock().unwrap().len(), 2);
}

#[test]
fn software_process_arming_timeout_keeps_the_child_owned_until_recovery_finishes() {
    let mut f = Fixture::new();
    f.hal().disarm_fan_guard().unwrap();
    let directory = f.dir.join("arming-timeout");
    fs::create_dir(&directory).unwrap();
    f.guards.lock().unwrap().push(directory.clone());
    let child_root = directory.clone();
    f.hal.as_mut().unwrap().test_fan_guard = Some(std::sync::Arc::new(move |policy| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_watchdog-fixture"));
        command.arg(&child_root).arg("arm-timeout");
        watchdog::Watchdog::spawn(command, false, policy)
    }));
    let hal = f.hal();
    let before = DEVICE.lock().unwrap().writes.len();
    {
        let _gate = hal.begin_write().unwrap();
        assert!(
            fanctl::set_mode(hal, 1)
                .unwrap_err()
                .to_string()
                .contains("握手失败")
        );
        assert!(hal.fan_guard_armed().unwrap());
        assert!(fanctl::set_mode(hal, 1).is_err());
    }
    assert!(
        hal.supervise_fan_guard()
            .unwrap_err()
            .to_string()
            .contains("仍在恢复")
    );
    assert_eq!(DEVICE.lock().unwrap().writes.len(), before);
    fs::write(directory.join("release-recovery"), "1").unwrap();
    f.wait_guard_exit();
    hal.supervise_fan_guard().unwrap();
    assert!(!hal.fan_guard_armed().unwrap());
    assert!(DEVICE.lock().unwrap().writes.len() > before);
}
