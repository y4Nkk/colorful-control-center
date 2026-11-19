//! 生产 apply/tick/恢复与真实独立监护组合；DCHU 为内存外部函数桩。
#![allow(dead_code, unused_imports)]
include!("../src/modules.rs");
#[path = "support/firmware_handoff_tests.rs"]
mod firmware_handoff_tests;
#[path = "support/software_lan_tests.rs"]
mod lan_tests;
#[path = "support/process.rs"]
mod process;
#[path = "support/software_safety_tests.rs"]
mod safety_tests;
use ccc_core::{
    fan::{CurvePoint, FanCurve},
    software_fan::Curves,
};
use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::Mutex,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

struct Device {
    pages: [[u8; 256]; 16],
    cpu: u8,
    writes: Vec<(i32, Vec<u8>)>,
    fail_duty: bool,
    fail_release: bool,
    journal_path: Option<PathBuf>,
    unjournaled: usize,
    hold_on_duty: bool,
    journal_hold: Option<fs::File>,
}
static DEVICE: Mutex<Device> = Mutex::new(Device {
    pages: [[0; 256]; 16],
    cpu: 50,
    writes: Vec::new(),
    fail_duty: false,
    fail_release: false,
    journal_path: None,
    unjournaled: 0,
    hold_on_duty: false,
    journal_hold: None,
});
static SERIAL: Mutex<()> = Mutex::new(());

unsafe extern "system" fn buffer(command: i32, out: *mut u8) -> i32 {
    let device = DEVICE.lock().unwrap();
    let mut p = [0; 257];
    match command {
        12 => {
            p[18] = device.cpu;
            p[16] = 76;
            p[19] = 76;
            p[2..4].copy_from_slice(&474u16.to_be_bytes());
        }
        13 => {
            p[12] = 2;
            p[50] = 3;
            for at in [16, 24] {
                p[at..at + 6].copy_from_slice(&[40, 51, 60, 102, 80, 204]);
            }
        }
        _ => return 0,
    }
    p[256] = p[0];
    unsafe {
        std::ptr::copy_nonoverlapping(p.as_ptr(), out, p.len());
    }
    command
}
unsafe extern "system" fn exchange(
    command: i32,
    input: *const u8,
    length: i32,
    out: *mut u8,
) -> i32 {
    assert_eq!((command, length, unsafe { *input }), (4, 256, 8));
    let device = DEVICE.lock().unwrap();
    unsafe {
        std::ptr::copy_nonoverlapping(device.pages[7].as_ptr(), out, 256);
        *out.add(256) = device.pages[7][0];
    }
    command
}
unsafe extern "system" fn read(page: i32, offset: i32, length: i32, out: *mut u8) -> i32 {
    let device = DEVICE.lock().unwrap();
    unsafe {
        std::ptr::copy_nonoverlapping(
            device.pages[page as usize][offset as usize..].as_ptr(),
            out,
            length as usize,
        );
    }
    4096
}
unsafe extern "system" fn write(page: i32, offset: i32, length: i32, data: *const u8) -> i32 {
    let bytes = unsafe { std::slice::from_raw_parts(data, length as usize) };
    let mut device = DEVICE.lock().unwrap();
    device.pages[page as usize][offset as usize..offset as usize + bytes.len()]
        .copy_from_slice(bytes);
    1
}
unsafe extern "system" fn set(command: i32, data: *const u8, length: i32) -> i32 {
    let bytes = unsafe { std::slice::from_raw_parts(data, length as usize) };
    let mut device = DEVICE.lock().unwrap();
    if command == 4 && bytes.get(6) == Some(&0xc1) {
        let active = device
            .journal_path
            .as_ref()
            .and_then(|path| fs::read(path).ok())
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
            .is_some_and(|record| record["active"] == true);
        if !active {
            device.unjournaled += 1;
        }
        if device.hold_on_duty {
            use std::os::windows::fs::OpenOptionsExt;
            device.hold_on_duty = false;
            device.journal_hold = device.journal_path.as_ref().and_then(|path| {
                fs::OpenOptions::new()
                    .read(true)
                    .share_mode(1)
                    .open(path)
                    .ok()
            });
        }
    }
    device.writes.push((command, bytes.to_vec()));
    if command == 4 && device.fail_duty {
        device.fail_duty = false;
        return 0;
    }
    if command == 0x69 && device.fail_release {
        device.fail_release = false;
        return 0;
    }
    command
}

fn curves() -> Curves {
    [(
        "cpu".into(),
        FanCurve {
            points: vec![
                CurvePoint {
                    temp_c: 40.0,
                    duty_pct: 30.0,
                },
                CurvePoint {
                    temp_c: 95.0,
                    duty_pct: 100.0,
                },
            ],
            hysteresis_c: 3.0,
            ramp_pct_per_s: 10.0,
        },
    )]
    .into()
}

struct Fixture {
    hal: Option<hal::Hal>,
    root: PathBuf,
    dir: PathBuf,
    guards: std::sync::Arc<Mutex<Vec<PathBuf>>>,
    _serial: std::sync::MutexGuard<'static, ()>,
}
impl Fixture {
    fn new() -> Self {
        let serial = SERIAL.lock().unwrap();
        *DEVICE.lock().unwrap() = Device {
            pages: [[0; 256]; 16],
            cpu: 50,
            writes: vec![],
            fail_duty: false,
            fail_release: false,
            journal_path: None,
            unjournaled: 0,
            hold_on_duty: false,
            journal_hold: None,
        };
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/software-tests");
        fs::create_dir_all(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let dir = root.join(format!(
            "{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&dir).unwrap();
        let io =
            hal::oem_dchu::Dchu::test_io(buffer, read, set, write).with_test_exchange(exchange);
        let mut hal = hal::Hal::test_instance(io, None);
        hal.bind_software_fan_record(dir.join(hal::SOFTWARE_FAN_RECORD_NAME));
        DEVICE.lock().unwrap().journal_path = Some(dir.join(hal::SOFTWARE_FAN_RECORD_NAME));
        let guards = std::sync::Arc::new(Mutex::new(Vec::new()));
        let owned = guards.clone();
        let owner = dir.clone();
        hal.test_fan_guard = Some(std::sync::Arc::new(move || {
            let mut directories = owned.lock().unwrap();
            let directory = if directories.is_empty() {
                owner.clone()
            } else {
                let directory = owner.join(format!("guard-{}", directories.len()));
                fs::create_dir(&directory)?;
                directory
            };
            let temperature = DEVICE.lock().unwrap().cpu;
            fs::write(directory.join("temperature"), temperature.to_string())?;
            let mut command = Command::new(env!("CARGO_BIN_EXE_watchdog-fixture"));
            command.arg(&directory).arg("sensor-file");
            let guard = watchdog::Watchdog::spawn(command)?;
            directories.push(directory);
            Ok(guard)
        }));
        Self {
            hal: Some(hal),
            root,
            dir,
            guards,
            _serial: serial,
        }
    }
    fn hal(&self) -> &hal::Hal {
        self.hal.as_ref().unwrap()
    }
    fn wait_file(&self, name: &str) {
        let deadline = Instant::now() + Duration::from_secs(8);
        while !self.dir.join(name).exists() {
            assert!(Instant::now() < deadline, "日志：{}", self.dir.display());
            std::thread::sleep(Duration::from_millis(15));
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.hal.take(); // 关闭管道；模拟监护最多一个采样周期后恢复并退出。
        if std::thread::panicking() {
            eprintln!("保留失败日志：{}", self.dir.display());
            return;
        }
        for directory in self.guards.lock().unwrap().iter() {
            let deadline = Instant::now() + Duration::from_secs(8);
            while !directory.join("child.exit").exists() {
                assert!(
                    Instant::now() < deadline,
                    "监护缺少退出日志：{}",
                    directory.display()
                );
                std::thread::sleep(Duration::from_millis(15));
            }
            let pid = fs::read_to_string(directory.join("child.pid"))
                .unwrap()
                .parse()
                .unwrap();
            while process::alive(pid) {
                assert!(
                    Instant::now() < deadline,
                    "监护未退出：{}",
                    directory.display()
                );
                std::thread::sleep(Duration::from_millis(15));
            }
        }
        let target = self.dir.canonicalize().unwrap();
        assert_eq!(target.parent(), Some(self.root.as_path()));
        fs::remove_dir_all(target).unwrap();
    }
}

#[test]
fn software_process_apply_tick_and_firmware_switch_share_production_paths() {
    let f = Fixture::new();
    let hal = f.hal();
    let _write = hal.begin_write().unwrap();
    fanctl::software::apply(hal, &curves()).unwrap();
    assert!(fanctl::read_profile(hal).unwrap().software.is_some());
    assert_eq!(DEVICE.lock().unwrap().writes[0].0, 4);
    drop(_write);
    fanctl::software::tick(hal).unwrap();
    let _write = hal.begin_write().unwrap();
    fanctl::set_mode(hal, 0).unwrap();
    assert!(fanctl::read_profile(hal).unwrap().software.is_none());
    drop(_write);
    let count = DEVICE.lock().unwrap().writes.len();
    fanctl::software::tick(hal).unwrap();
    assert_eq!(DEVICE.lock().unwrap().writes.len(), count);
    assert!(
        DEVICE
            .lock()
            .unwrap()
            .writes
            .iter()
            .any(|(cmd, b)| *cmd == 0x69 && b == &3u32.to_le_bytes())
    );
}

#[test]
fn software_process_failed_release_retries_through_parent_supervision() {
    let f = Fixture::new();
    let hal = f.hal();
    {
        let _write = hal.begin_write().unwrap();
        fanctl::software::apply(hal, &curves()).unwrap();
    }
    {
        let mut d = DEVICE.lock().unwrap();
        d.fail_duty = true;
        d.fail_release = true;
    }
    assert!(
        fanctl::software::tick(hal)
            .unwrap_err()
            .to_string()
            .contains("自动恢复失败")
    );
    assert!(fanctl::software::read(hal).unwrap().is_none());
    f.wait_guard_exit();
    hal.supervise_fan_guard().unwrap();
    assert!(hal.safety_status().contains("已恢复"));
    assert_eq!(
        DEVICE
            .lock()
            .unwrap()
            .writes
            .iter()
            .filter(|(c, _)| *c == 0x69)
            .count(),
        2
    );
    assert!(DEVICE.lock().unwrap().writes.iter().any(|(c, _)| *c == 121));
}

#[test]
fn software_process_stalled_loop_closes_real_guard_pipe_and_cancels_output() {
    let f = Fixture::new();
    let hal = f.hal();
    {
        let _write = hal.begin_write().unwrap();
        fanctl::software::apply(hal, &curves()).unwrap();
    }
    *hal.software_progress.lock().unwrap() = Some(Instant::now() - Duration::from_secs(4));
    assert!(
        hal.pulse_fan_guard()
            .unwrap_err()
            .to_string()
            .contains("停止心跳")
    );
    f.wait_file("restore.log");
    f.wait_guard_exit();
    hal.supervise_fan_guard().unwrap();
    let record: serde_json::Value =
        serde_json::from_slice(&fs::read(f.dir.join(hal::SOFTWARE_FAN_RECORD_NAME)).unwrap())
            .unwrap();
    assert_eq!(record["active"], false);
    assert_eq!(record["faulted"], true);
    let count = DEVICE.lock().unwrap().writes.len();
    fanctl::software::tick(hal).unwrap();
    assert_eq!(DEVICE.lock().unwrap().writes.len(), count);
}

#[test]
fn software_process_profile_rollback_restarts_the_exact_previous_curve() {
    let f = Fixture::new();
    let hal = f.hal();
    let _write = hal.begin_write().unwrap();
    let wanted = curves();
    fanctl::software::apply(hal, &wanted).unwrap();
    let before = fanctl::checkpoint(hal, false).unwrap();
    fanctl::set_mode(hal, 1).unwrap();
    assert!(fanctl::software::read(hal).unwrap().is_none());
    fanctl::restore_checkpoint(hal, &before).unwrap();
    assert_eq!(fanctl::software::read(hal).unwrap().unwrap(), wanted);
    fanctl::set_mode(hal, 0).unwrap();
}

fn configure_options(f: &mut Fixture, mode: u8, turbo: u8) {
    f.hal
        .as_mut()
        .unwrap()
        .set_test_dtt_driver(std::sync::Arc::new(|| Ok(true)));
    let mut state = DEVICE.lock().unwrap();
    state.pages[7][0] = 1;
    state.pages[7][11] = 16;
    state.pages[7][16] = 16;
    state.pages[7][17] = 15;
    state.pages[1][1] = mode;
    state.pages[4][8] = turbo;
}

#[test]
fn software_process_options_temporarily_release_and_resume_the_original_curve() {
    let mut f = Fixture::new();
    configure_options(&mut f, 2, 0);
    let hal = f.hal();
    let _gate = hal.begin_write().unwrap();
    let wanted = curves();
    fanctl::software::apply(hal, &wanted).unwrap();
    let values = powerctl::mode::boost::Values {
        turbo_fan: Some(false),
        dtt: Some(true),
    };
    let plan = powerctl::mode::boost::Plan::prepare(hal, &values).unwrap();
    let mut changed = powerctl::mode::boost::Change::default();
    plan.execute(hal, true, &mut changed).unwrap();
    assert_eq!(fanctl::software::read(hal).unwrap(), Some(wanted.clone()));
    assert_eq!(DEVICE.lock().unwrap().pages[1][32], 1);
    assert!(
        DEVICE
            .lock()
            .unwrap()
            .writes
            .iter()
            .any(|(command, _)| *command == 0x69)
    );
    drop(_gate);
    fanctl::software::tick(hal).unwrap();
    let _gate = hal.begin_write().unwrap();
    plan.rollback(hal, &mut changed, true).unwrap();
    assert_eq!(fanctl::software::read(hal).unwrap(), Some(wanted));
    assert_eq!(DEVICE.lock().unwrap().pages[1][32], 0);
    assert!(!changed.dirty());
    fanctl::set_mode(hal, 0).unwrap();
}

#[test]
fn software_process_inactive_saved_options_do_not_interrupt_the_running_curve() {
    let mut f = Fixture::new();
    configure_options(&mut f, 3, 0);
    let hal = f.hal();
    let _gate = hal.begin_write().unwrap();
    let wanted = curves();
    fanctl::software::apply(hal, &wanted).unwrap();
    let before = DEVICE.lock().unwrap().writes.len();
    let values = powerctl::mode::boost::Values {
        turbo_fan: Some(true),
        dtt: Some(true),
    };
    let plan = powerctl::mode::boost::Plan::prepare(hal, &values).unwrap();
    let mut changed = powerctl::mode::boost::Change::default();
    plan.execute(hal, true, &mut changed).unwrap();
    assert_eq!(DEVICE.lock().unwrap().writes.len(), before);
    assert_eq!(fanctl::software::read(hal).unwrap(), Some(wanted.clone()));
    plan.rollback(hal, &mut changed, true).unwrap();
    assert_eq!(DEVICE.lock().unwrap().writes.len(), before);
    assert_eq!(fanctl::software::read(hal).unwrap(), Some(wanted));
    fanctl::set_mode(hal, 0).unwrap();
}

#[test]
fn software_process_turbo_off_can_precede_firmware_custom_with_the_same_recovery_baseline() {
    let mut f = Fixture::new();
    configure_options(&mut f, 2, 1);
    let hal = f.hal();
    let _gate = hal.begin_write().unwrap();
    let values = powerctl::mode::boost::Values {
        turbo_fan: Some(false),
        dtt: Some(false),
    };
    let plan = powerctl::mode::boost::Plan::prepare(hal, &values).unwrap();
    let mut changed = powerctl::mode::boost::Change::default();
    plan.execute(hal, false, &mut changed).unwrap();
    fanctl::set_mode(hal, 6).unwrap();
    {
        let state = DEVICE.lock().unwrap();
        assert_eq!(state.pages[4][8], 0);
        assert_eq!(state.pages[4][5], 6);
    }
    plan.rollback(hal, &mut changed, true).unwrap();
    {
        let state = DEVICE.lock().unwrap();
        assert_eq!(state.pages[4][8], 1);
        assert_eq!(state.pages[4][5], 0);
    }
    assert!(!changed.dirty());
}
