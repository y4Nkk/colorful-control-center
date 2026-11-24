use super::*;
use std::{collections::BTreeSet, sync::Arc};

struct State {
    current: Snapshot,
    writes: usize,
    fail: BTreeSet<usize>,
    ignore: BTreeSet<usize>,
    owner: bool,
}
struct Memory(Mutex<State>);
impl Memory {
    fn new(saved: u8) -> Arc<Self> {
        Arc::new(Self(Mutex::new(State {
            current: Snapshot {
                devices: vec!["ACPI\\I2C0001\\0".into()],
                enabled: Some(true),
                saved,
            },
            writes: 0,
            fail: BTreeSet::new(),
            ignore: BTreeSet::new(),
            owner: false,
        })))
    }
    fn change(&self, edit: impl FnOnce(&mut State)) {
        edit(&mut self.0.lock().unwrap());
    }
    fn current(&self) -> Snapshot {
        self.0.lock().unwrap().current.clone()
    }
}
impl Io for Memory {
    fn snapshot(&self) -> Result<Snapshot> {
        Ok(self.current())
    }
    fn ensure_available(&self) -> Result<()> {
        if self.0.lock().unwrap().owner {
            bail!("原厂控制者存在");
        }
        Ok(())
    }
    fn set_enabled(&self, devices: &[String], before: bool, enabled: bool) -> Result<()> {
        let mut s = self.0.lock().unwrap();
        if s.current.devices != devices || s.current.enabled != Some(before) {
            bail!("旧状态");
        }
        s.writes += 1;
        let step = s.writes;
        if !s.ignore.contains(&step) {
            s.current.enabled = Some(enabled);
        }
        if s.fail.contains(&step) {
            bail!("模拟切换失败");
        }
        Ok(())
    }
    fn save(&self, value: u8) -> Result<()> {
        let mut s = self.0.lock().unwrap();
        s.writes += 1;
        let step = s.writes;
        if !s.ignore.contains(&step) {
            s.current.saved = value;
        }
        if s.fail.contains(&step) {
            bail!("模拟保存失败");
        }
        Ok(())
    }
}
fn test_hal() -> Hal {
    unsafe extern "system" fn get(_: i32, _: *mut u8) -> i32 {
        0
    }
    unsafe extern "system" fn read(_: i32, _: i32, _: i32, _: *mut u8) -> i32 {
        0
    }
    unsafe extern "system" fn set(_: i32, _: *const u8, _: i32) -> i32 {
        0
    }
    unsafe extern "system" fn save(_: i32, _: i32, _: i32, _: *const u8) -> i32 {
        0
    }
    Hal::test_instance(
        crate::hal::oem_dchu::Dchu::test_io(get, read, set, save),
        None,
    )
}
fn setup(saved: u8) -> (Hal, Controller, Arc<Memory>) {
    let io = Memory::new(saved);
    (
        test_hal(),
        Controller::default().with_test_io(io.clone()),
        io,
    )
}
fn apply(hal: &Hal, controller: &Controller, enabled: bool) -> Result<()> {
    controller.apply(
        hal,
        &Request {
            expected_revision: controller.read(hal)?.revision,
            enabled,
        },
    )
}
#[test]
fn read_never_writes_and_capture_refuses_unknown_preference() {
    let (hal, controller, io) = setup(77);
    let before = io.current();
    assert_eq!(controller.read(&hal).unwrap().current, before);
    assert_eq!(io.0.lock().unwrap().writes, 0);
    assert!(controller.scope().unwrap().capture(&hal).is_err());
    apply(&hal, &controller, false).unwrap();
    assert_eq!(
        io.current(),
        Snapshot {
            devices: before.devices,
            enabled: Some(false),
            saved: 0,
        }
    );
}
#[test]
fn no_change_has_no_writes() {
    let (hal, controller, io) = setup(1);
    apply(&hal, &controller, true).unwrap();
    assert_eq!(io.0.lock().unwrap().writes, 0);
}
#[test]
fn stale_preview_missing_or_replaced_device_and_owner_produce_no_output() {
    let (hal, controller, io) = setup(1);
    let revision = controller.read(&hal).unwrap().revision;
    io.change(|s| s.current.enabled = Some(false));
    assert!(
        controller
            .apply(
                &hal,
                &Request {
                    expected_revision: revision,
                    enabled: true
                }
            )
            .is_err()
    );
    for bad in [None, Some(false)] {
        io.change(|s| {
            s.current.enabled = bad;
            s.current.devices.clear();
        });
        assert!(apply(&hal, &controller, true).is_err());
    }
    io.change(|s| {
        s.current = Memory::new(1).current();
        s.owner = true;
    });
    assert!(apply(&hal, &controller, false).is_err());
    assert_eq!(io.0.lock().unwrap().writes, 0);
}
#[test]
fn every_partial_write_and_ignored_success_is_verified_and_compensated() {
    for step in 1..=2 {
        for ignored in [false, true] {
            let (hal, controller, io) = setup(1);
            let before = io.current();
            io.change(|s| {
                if ignored {
                    s.ignore.insert(step);
                } else {
                    s.fail.insert(step);
                }
            });
            assert!(apply(&hal, &controller, false).is_err());
            assert_eq!(io.current(), before, "step {step} ignored {ignored}");
        }
    }
}
#[test]
fn failed_compensation_leaves_actual_state_and_control_can_retry() {
    let (hal, controller, io) = setup(1);
    let before = io.current();
    io.change(|s| s.fail.extend([2, 3]));
    assert!(apply(&hal, &controller, false).is_err());
    io.change(|s| s.fail.clear());
    apply(&hal, &controller, true).unwrap();
    assert_eq!(io.current(), before);
}
#[test]
fn scope_plan_preserves_mixed_current_and_preference_and_rolls_back_once() {
    let (hal, controller, io) = setup(0);
    let before = io.current();
    let scope = controller.scope().unwrap();
    let target = scope.capture(&hal).unwrap().unwrap();
    assert!(target.enabled && !target.saved_enabled);
    let plan = scope
        .prepare(
            &hal,
            &Target {
                enabled: false,
                saved_enabled: true,
                ..target
            },
        )
        .unwrap();
    let mut needs = false;
    scope.execute(&hal, &plan, &mut needs).unwrap();
    assert!(needs);
    scope.rollback(&hal, &plan).unwrap();
    let writes = io.0.lock().unwrap().writes;
    scope.rollback(&hal, &plan).unwrap();
    assert_eq!(io.0.lock().unwrap().writes, writes);
    assert_eq!(io.current(), before);
}
#[test]
fn changed_devices_do_not_block_control_and_hotkey_binds_new_identity() {
    let (hal, controller, io) = setup(1);
    apply(&hal, &controller, false).unwrap();
    io.change(|s| s.current.devices.push("ACPI\\I2C0002\\0".into()));
    assert!(controller.read(&hal).unwrap().blocked_reason.is_none());
    let identity = controller.hotkey_identity(&hal).unwrap();
    assert!(controller.toggle(&hal, "stale").is_err());
    assert!(controller.toggle(&hal, &identity).unwrap());
    assert_eq!(io.current().enabled, Some(true));
    assert_eq!(io.current().devices.len(), 2);
    apply(&hal, &controller, false).unwrap();
    assert_eq!(io.current().enabled, Some(false));
}
#[test]
fn toggle_uses_real_current_not_saved_preference_and_requires_same_identity() {
    let (hal, controller, io) = setup(0);
    let identity = controller.hotkey_identity(&hal).unwrap();
    assert!(controller.toggle(&hal, "stale").is_err());
    assert!(!controller.toggle(&hal, &identity).unwrap());
    assert_eq!(io.current().enabled, Some(false));
    assert!(controller.toggle(&hal, &identity).unwrap());
    assert_eq!(io.current().saved, 1);
    assert!(!controller.toggle(&hal, &identity).unwrap());
    assert_eq!(io.current().saved, 0);
}
#[test]
fn target_and_provider_are_strict() {
    let hal = test_hal();
    assert!(
        Controller::default()
            .read(&hal)
            .err()
            .unwrap()
            .to_string()
            .contains("禁止回退")
    );
    let io = Memory::new(1);
    let mut target = Target {
        devices: io.current().devices,
        enabled: false,
        saved_enabled: false,
    };
    target.validate().unwrap();
    target.devices.push(target.devices[0].clone());
    assert!(target.validate().is_err());
    assert!(
        serde_json::from_value::<Target>(serde_json::json!({
            "devices": ["x"], "enabled": true, "savedEnabled": true, "unused": 0
        }))
        .is_err()
    );
}
