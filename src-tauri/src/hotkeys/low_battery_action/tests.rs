use super::*;
use std::cell::{Cell, RefCell};

struct Data {
    plan: u128,
    actions: [u32; 2],
    power: PowerStatus,
    signals: Vec<u8>,
    sleeps: Vec<bool>,
    reject: bool,
}
struct Memory {
    data: RefCell<Data>,
    locked: Cell<bool>,
    known: Cell<bool>,
    writable: Cell<bool>,
}
impl Memory {
    fn new() -> Self {
        Self {
            data: RefCell::new(Data {
                plan: 1,
                actions: [1, 2],
                power: PowerStatus {
                    present: Some(true),
                    ac_online: Some(false),
                    percent: Some(10),
                    ..PowerStatus::default()
                },
                signals: vec![],
                sleeps: vec![],
                reject: false,
            }),
            locked: Cell::new(false),
            known: Cell::new(true),
            writable: Cell::new(true),
        }
    }
    fn identity(&self) -> String {
        read(self).identity.unwrap()
    }
}
impl Io for Memory {
    fn locked<T>(&self, f: impl FnOnce() -> Result<T>) -> Result<T> {
        assert!(!self.locked.replace(true));
        let result = f();
        self.locked.set(false);
        result
    }
    fn known_firmware(&self) -> bool {
        self.known.get()
    }
    fn active(&self) -> Result<u128> {
        assert!(self.locked.get());
        Ok(self.data.borrow().plan)
    }
    fn action(&self, _: u128, action: BatteryAction) -> Result<u32> {
        assert!(self.locked.get());
        Ok(self.data.borrow().actions[usize::from(action == BatteryAction::Critical)])
    }
    fn power(&self) -> Result<PowerStatus> {
        Ok(self.data.borrow().power.clone())
    }
    fn signal(&self, level: u8) -> Result<()> {
        assert!(self.locked.get());
        self.data.borrow_mut().signals.push(level);
        Ok(())
    }
    fn ensure_writable(&self) -> Result<()> {
        if !self.writable.get() {
            bail!("隔离校准占用");
        }
        Ok(())
    }
    fn suspend(&self, hibernate: bool) -> Result<()> {
        assert!(!self.locked.get(), "系统动作不应持有HAL锁");
        let mut data = self.data.borrow_mut();
        data.sleeps.push(hibernate);
        if data.reject {
            bail!("隔离系统拒绝动作");
        }
        Ok(())
    }
}

#[test]
fn events_read_the_right_plan_action_and_reproduce_original_noop_values() {
    for (event, index) in [(44, 0), (45, 1)] {
        for value in 0..=3 {
            let io = Memory::new();
            io.data.borrow_mut().actions[index] = value;
            let identity = io.identity();
            let result = run(&io, &identity, event, || Ok(()), || Ok(true))
                .unwrap()
                .unwrap();
            assert_eq!(io.data.borrow().signals, [event - 43]);
            assert_eq!(
                io.data.borrow().sleeps,
                match value {
                    1 => vec![false],
                    2 => vec![true],
                    _ => vec![],
                }
            );
            if value == 3 {
                assert!(result.contains("不执行系统操作"));
            }
        }
    }
}

#[test]
fn ac_unknown_or_absent_battery_never_requests_a_system_action() {
    for (on_ac, present, succeeds) in [
        (Some(true), Some(true), true),
        (None, Some(true), false),
        (Some(false), None, false),
        (Some(false), Some(false), false),
    ] {
        let io = Memory::new();
        let identity = io.identity();
        {
            let mut data = io.data.borrow_mut();
            data.power.ac_online = on_ac;
            data.power.present = present;
        }
        assert_eq!(
            run(&io, &identity, 44, || Ok(()), || Ok(true)).is_ok(),
            succeeds
        );
        assert!(io.data.borrow().sleeps.is_empty());
    }
}

#[test]
fn unconfirmed_battery_is_not_presented_as_battery_power() {
    let io = Memory::new();
    io.data.borrow_mut().power.present = None;
    assert_eq!(read(&io).on_ac, None);
    io.data.borrow_mut().power.ac_online = Some(true);
    assert_eq!(read(&io).on_ac, Some(true));
}

#[test]
fn cancellation_or_policy_change_during_wait_never_sleeps() {
    let io = Memory::new();
    let identity = io.identity();
    assert!(
        run(&io, &identity, 44, || Ok(()), || Ok(false))
            .unwrap()
            .is_none()
    );
    assert!(io.data.borrow().sleeps.is_empty());
    assert!(
        run(
            &io,
            &identity,
            44,
            || Ok(()),
            || {
                io.data.borrow_mut().actions[0] = 2;
                Ok(true)
            }
        )
        .is_err()
    );
    assert!(io.data.borrow().sleeps.is_empty());
    io.data.borrow_mut().actions[0] = 1;
    assert!(
        run(
            &io,
            &identity,
            44,
            || Ok(()),
            || {
                io.data.borrow_mut().plan = 2;
                Ok(true)
            }
        )
        .is_err()
    );
    assert!(io.data.borrow().sleeps.is_empty());
}

#[test]
fn rechecks_permission_and_supply_immediately_before_sleep() {
    let io = Memory::new();
    let identity = io.identity();
    let checks = Cell::new(0);
    let result = run(
        &io,
        &identity,
        44,
        || {
            checks.set(checks.get() + 1);
            if checks.get() == 5 {
                io.data.borrow_mut().power.ac_online = Some(true);
            }
            Ok(())
        },
        || Ok(true),
    )
    .unwrap()
    .unwrap();
    assert!(result.contains("已接电"));
    assert!(io.data.borrow().sleeps.is_empty());
    io.data.borrow_mut().power.ac_online = Some(false);
    checks.set(0);
    assert!(
        run(
            &io,
            &identity,
            44,
            || {
                checks.set(checks.get() + 1);
                if checks.get() == 4 {
                    bail!("许可撤销")
                }
                Ok(())
            },
            || Ok(true)
        )
        .is_err()
    );
    assert!(io.data.borrow().sleeps.is_empty());
}

#[test]
fn unsupported_state_and_system_rejection_are_reported() {
    let io = Memory::new();
    io.known.set(false);
    assert!(read(&io).identity.is_none());
    io.known.set(true);
    io.data.borrow_mut().actions[0] = 4;
    assert!(read(&io).identity.is_none());
    io.data.borrow_mut().actions[0] = 1;
    let identity = io.identity();
    io.writable.set(false);
    assert!(run(&io, &identity, 44, || Ok(()), || Ok(true)).is_err());
    assert!(io.data.borrow().signals.is_empty());
    io.writable.set(true);
    io.data.borrow_mut().reject = true;
    assert!(
        format!(
            "{:#}",
            run(&io, &identity, 44, || Ok(()), || Ok(true)).unwrap_err()
        )
        .contains("隔离系统拒绝")
    );
}
