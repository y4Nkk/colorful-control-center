use super::*;
use std::sync::{Arc, Mutex};
#[derive(Clone)]
struct Fake(Arc<Mutex<Data>>);
struct Data {
    devices: Vec<Device>,
    calls: usize,
    fail: Vec<usize>,
    ignore: bool,
    partial: bool,
}
fn device() -> Device {
    Device {
        id: "ACPI\\NVDA0820\\4&1".into(),
        name: "NVPCF".into(),
        enabled: true,
        automatic_power_off: false,
        problem: 0,
        drives_display: false,
        can_disable: true,
    }
}
fn setup() -> (Controller, Fake) {
    let io = Fake(Arc::new(Mutex::new(Data {
        devices: vec![device()],
        calls: 0,
        fail: vec![],
        ignore: false,
        partial: false,
    })));
    (Controller::default(), io)
}
impl Access for Fake {
    fn list(&self) -> Result<Vec<Device>> {
        Ok(self.0.lock().unwrap().devices.clone())
    }
    fn set(&self, expected: &Device, target: Target) -> Result<()> {
        let mut data = self.0.lock().unwrap();
        data.calls += 1;
        let failed = data.fail.contains(&data.calls);
        if !data.ignore && (!failed || data.partial) {
            let d = data
                .devices
                .iter_mut()
                .find(|d| d.id == expected.id)
                .unwrap();
            d.enabled = target.enabled;
            d.automatic_power_off = target.automatic_power_off;
            d.problem = if target.enabled { 0 } else { 22 };
        }
        if failed {
            bail!("原厂工作进程未执行或补偿失败");
        }
        Ok(())
    }
}
#[test]
fn stale_output_and_fault_states_never_execute() {
    let (c, io) = setup();
    let mut stale = device();
    stale.name = "old".into();
    assert!(
        c.apply(&io, &stale, Target::requested(false), false)
            .is_err()
    );
    io.0.lock().unwrap().devices[0].can_disable = false;
    let now = io.list().unwrap()[0].clone();
    assert!(c.apply(&io, &now, Target::requested(false), false).is_err());
    assert_eq!(io.0.lock().unwrap().calls, 0);
    assert!(!c.read(&io).unwrap().can_restore);
}
#[test]
fn failed_authorized_write_does_not_launch_unnecessary_recovery() {
    let (c, io) = setup();
    io.0.lock().unwrap().fail = vec![1];
    assert!(
        c.apply(&io, &device(), Target::requested(false), false)
            .is_err()
    );
    assert_eq!(io.0.lock().unwrap().calls, 1);
    assert!(io.list().unwrap()[0].enabled);
    assert!(c.read(&io).unwrap().can_restore);
}
#[test]
fn ignored_write_fails_readback_and_keeps_baseline() {
    let (c, io) = setup();
    io.0.lock().unwrap().ignore = true;
    assert!(
        c.apply(&io, &device(), Target::requested(false), false)
            .unwrap_err()
            .to_string()
            .contains("回读不一致")
    );
    assert!(c.read(&io).unwrap().can_restore);
}
#[test]
fn partial_failure_keeps_first_baseline_and_current_apply_can_retry() {
    let (c, io) = setup();
    {
        let mut d = io.0.lock().unwrap();
        d.fail = vec![1];
        d.partial = true;
    }
    assert!(
        c.apply(&io, &device(), Target::requested(false), false)
            .is_err()
    );
    assert_eq!(
        io.0.lock().unwrap().calls,
        1,
        "补偿在工作进程内，父进程不重复弹授权"
    );
    let current = io.list().unwrap()[0].clone();
    c.apply(&io, &current, Target::requested(true), true)
        .unwrap();
    assert!(io.list().unwrap()[0].enabled);
    c.apply(&io, &device(), Target::requested(false), false)
        .unwrap();
    let off = io.list().unwrap()[0].clone();
    c.apply(&io, &off, Target::requested(true), false).unwrap();
    let current = io.list().unwrap()[0].clone();
    c.apply(&io, &current, Target::requested(true), true)
        .unwrap();
    assert!(io.list().unwrap()[0].enabled);
}
#[test]
fn runtime_blocks_uac_while_refresh_trial_needs_automatic_recovery() {
    let fixture = crate::display::test_fixture::Fixture::new();
    fixture.enable();
    let display = fixture.controller;
    let original = display.read().unwrap().displays[0].current.clone();
    display.apply(&original, 144, 0).unwrap();
    let hal = super::test_fixture::hal();
    let clocks = crate::gpuctl::GpuController::new();
    let (controller, io) = setup();
    assert!(
        controller
            .apply_runtime(&hal, &clocks, &display, &device(), false)
            .unwrap_err()
            .to_string()
            .contains("刷新率试用")
    );
    assert!(controller.restore_runtime(&hal, &clocks, &display).is_err());
    assert_eq!(io.0.lock().unwrap().calls, 0);
}
