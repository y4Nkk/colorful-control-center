use super::*;
use std::cell::{Cell, RefCell};

struct Memory {
    snapshot: RefCell<Snapshot>,
    on_ac: Cell<Option<bool>>,
    locked: Cell<bool>,
    requests: RefCell<Vec<Kind>>,
    fail: Cell<bool>,
    forbid_lid: Cell<bool>,
}
impl Memory {
    fn new(values: [u32; 2], modern: bool, on_ac: bool) -> Self {
        Self {
            snapshot: RefCell::new(Snapshot::new(1, values, modern).unwrap()),
            on_ac: Cell::new(Some(on_ac)),
            locked: Cell::new(false),
            requests: RefCell::new(vec![]),
            fail: Cell::new(false),
            forbid_lid: Cell::new(false),
        }
    }
    fn identity(&self) -> String {
        self.snapshot.borrow().identity().unwrap()
    }
}
struct Unlock<'a>(&'a Cell<bool>);
impl Drop for Unlock<'_> {
    fn drop(&mut self) {
        self.0.set(false);
    }
}
impl Io for Memory {
    fn locked<T>(&self, f: impl FnOnce() -> Result<T>) -> Result<T> {
        assert!(!self.locked.replace(true));
        let _unlock = Unlock(&self.locked);
        f()
    }
    fn snapshot(&self) -> Result<Snapshot> {
        assert!(!self.forbid_lid.get());
        Ok(self.snapshot.borrow().clone())
    }
    fn on_ac(&self) -> Result<bool> {
        assert!(!self.forbid_lid.get());
        self.on_ac
            .get()
            .ok_or_else(|| anyhow::anyhow!("power unknown"))
    }
    fn ensure_writable(&self) -> Result<()> {
        Ok(())
    }
    fn request(&self, kind: Kind) -> Result<()> {
        assert!(!self.locked.get(), "系统调用不能持有HAL锁等待");
        self.requests.borrow_mut().push(kind);
        if self.fail.get() {
            bail!("system request denied");
        }
        Ok(())
    }
}

#[test]
fn lid_uses_actual_supply_and_plan_with_the_original_modern_standby_branch() {
    for modern in [false, true] {
        for on_ac in [false, true] {
            for value in 0..=3 {
                let values = if on_ac { [value, 0] } else { [0, value] };
                let io = Memory::new(values, modern, on_ac);
                let waited = Cell::new(false);
                run(
                    &io,
                    192,
                    &io.identity(),
                    || Ok(()),
                    |delay| {
                        assert_eq!(delay, Duration::from_millis(200));
                        waited.set(true);
                        Ok(true)
                    },
                )
                .unwrap();
                let expected = match value {
                    0 => None,
                    1 if modern => Some(Kind::ScreenOff),
                    1 => Some(Kind::Sleep),
                    2 => Some(Kind::Hibernate),
                    _ => Some(Kind::Shutdown),
                };
                assert_eq!(
                    *io.requests.borrow(),
                    expected.into_iter().collect::<Vec<_>>()
                );
                assert_eq!(waited.get(), expected == Some(Kind::ScreenOff));
            }
        }
    }
    assert!(Snapshot::new(1, [4, 0], false).is_err());
}

#[test]
fn shutdown_never_borrows_lid_or_low_battery_authorization() {
    let io = Memory::new([0, 0], false, true);
    io.forbid_lid.set(true);
    assert!(run(&io, 72, "battery-low-action-1", || Ok(()), |_| panic!()).is_err());
    run(&io, 72, SHUTDOWN_IDENTITY, || Ok(()), |_| panic!()).unwrap();
    assert_eq!(*io.requests.borrow(), [Kind::Shutdown]);
}

#[test]
fn stale_plan_unknown_power_and_supply_changes_prevent_all_requests() {
    let io = Memory::new([1, 2], true, true);
    assert!(run(&io, 192, "stale", || Ok(()), |_| panic!()).is_err());
    io.on_ac.set(None);
    assert!(run(&io, 192, &io.identity(), || Ok(()), |_| panic!()).is_err());
    io.on_ac.set(Some(true));
    assert!(
        run(
            &io,
            192,
            &io.identity(),
            || Ok(()),
            |_| {
                io.on_ac.set(Some(false));
                Ok(true)
            }
        )
        .is_err()
    );
    assert!(io.requests.borrow().is_empty());
}

#[test]
fn cancellation_at_each_boundary_and_during_display_delay_never_outputs() {
    for boundary in 1..=3 {
        let io = Memory::new([1, 0], true, true);
        let calls = Cell::new(0);
        assert!(
            run(
                &io,
                192,
                &io.identity(),
                || {
                    calls.set(calls.get() + 1);
                    if calls.get() == boundary {
                        bail!("cancelled");
                    }
                    Ok(())
                },
                |_| Ok(true)
            )
            .is_err()
        );
        assert!(io.requests.borrow().is_empty());
    }
    let io = Memory::new([1, 0], true, true);
    assert!(
        run(&io, 192, &io.identity(), || Ok(()), |_| Ok(false))
            .unwrap()
            .is_none()
    );
    assert!(io.requests.borrow().is_empty());
}

#[test]
fn system_denial_remains_visible_and_native_test_operations_are_forbidden() {
    let io = Memory::new([3, 0], false, true);
    io.fail.set(true);
    assert!(
        run(&io, 192, &io.identity(), || Ok(()), |_| panic!())
            .unwrap_err()
            .to_string()
            .contains("request denied")
    );
    assert_eq!(io.requests.borrow().len(), 1);
    for kind in [
        Kind::Sleep,
        Kind::Hibernate,
        Kind::ScreenOff,
        Kind::Shutdown,
    ] {
        assert!(crate::hal::system_action::request(kind).is_err());
    }
    assert!(crate::hal::system_action::modern_standby().is_err());
}
