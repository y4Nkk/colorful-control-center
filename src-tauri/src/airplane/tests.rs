use super::*;
use std::cell::{Cell, RefCell};
struct Fake {
    current: RefCell<Snapshot>,
    log: RefCell<Vec<String>>,
    fail: Cell<bool>,
    blocked: Cell<bool>,
}
impl Fake {
    fn new() -> Self {
        Self {
            current: RefCell::new(Snapshot {
                device: "device-a".into(),
                enabled: false,
            }),
            log: RefCell::new(vec![]),
            fail: Cell::new(false),
            blocked: Cell::new(false),
        }
    }
}
impl Io for Fake {
    fn require_device(&self, source: &Snapshot) -> Result<()> {
        self.current.borrow().require_device(source)
    }
    fn current(&self) -> Result<Snapshot> {
        Ok(self.current.borrow().clone())
    }
    fn ensure_writable(&self) -> Result<()> {
        if self.blocked.get() {
            bail!("隔离原厂进程占用");
        }
        Ok(())
    }
    fn firmware(&self, enabled: bool) -> Result<()> {
        self.log.borrow_mut().push(format!("firmware:{enabled}"));
        Ok(())
    }
    fn system(&self, _: &Snapshot, enabled: bool) -> Result<()> {
        self.log.borrow_mut().push(format!("system:{enabled}"));
        if self.fail.replace(false) {
            bail!("injected system failure");
        }
        self.current.borrow_mut().enabled = enabled;
        Ok(())
    }
    fn observed(
        &self,
        _: &Snapshot,
        enabled: bool,
        permit: &mut dyn FnMut() -> Result<()>,
    ) -> Result<()> {
        permit()?;
        if self.current.borrow().enabled != enabled {
            bail!("not observed");
        }
        Ok(())
    }
}
#[test]
fn reading_and_preview_never_write() {
    let io = Fake::new();
    let frame = profile(&io).unwrap();
    assert!(frame.revision.is_some());
    assert!(io.log.borrow().is_empty());
}
#[test]
fn ownership_blocker_is_visible_without_discarding_actual_radio_state() {
    let io = Fake::new();
    let before = io.current().unwrap();
    io.blocked.set(true);
    let frame = profile(&io).unwrap();
    assert_eq!(frame.current, Some(before));
    assert_eq!(frame.blocked_reason.as_deref(), Some("隔离原厂进程占用"));
    assert!(io.log.borrow().is_empty());
}
#[test]
fn absolute_protocol_writes_both_channels_for_each_change() {
    let io = Fake::new();
    transaction::apply(&io, io.current().unwrap(), true, || Ok(())).unwrap();
    transaction::apply(&io, io.current().unwrap(), false, || Ok(())).unwrap();
    assert_eq!(
        *io.log.borrow(),
        [
            "firmware:true",
            "system:true",
            "firmware:false",
            "system:false"
        ]
    );
}
#[test]
fn partial_failure_compensates_both_channels() {
    let io = Fake::new();
    io.fail.set(true);
    let error =
        transaction::apply(&io, io.current().unwrap(), true, || Ok(())).unwrap_err();
    assert!(format!("{error:#}").contains("injected system failure"));
    assert_eq!(
        *io.log.borrow(),
        [
            "firmware:true",
            "system:true",
            "firmware:false",
            "system:false"
        ]
    );
    assert!(!io.current().unwrap().enabled);
}
#[test]
fn cancellation_and_stale_state_before_output_never_write() {
    let io = Fake::new();
    assert!(
        transaction::apply(&io, io.current().unwrap(), true, || bail!(
            "cancelled"
        ))
        .is_err()
    );
    let mut stale = io.current().unwrap();
    stale.enabled = true;
    assert!(
        transaction::apply(&io, stale, true, || Ok(())).is_err()
    );
    assert!(io.log.borrow().is_empty());
}
#[test]
fn device_change_changes_identity() {
    let io = Fake::new();
    let before = io.current().unwrap();
    io.current.borrow_mut().device = "device-b".into();
    assert_ne!(
        before.identity().unwrap(),
        io.current().unwrap().identity().unwrap()
    );
}
#[test]
fn cancellation_after_firmware_compensates() {
    let io = Fake::new();
    let mut calls = 0;
    assert!(
        transaction::apply(&io, io.current().unwrap(), true, || {
            calls += 1;
            if calls == 3 {
                bail!("cancelled");
            }
            Ok(())
        })
        .is_err()
    );
    assert_eq!(
        *io.log.borrow(),
        ["firmware:true", "firmware:false", "system:false"]
    );
}
