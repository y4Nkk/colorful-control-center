use super::{
    codec::{self, Io},
    codec_store::Store,
};
use crate::hal::beep_audio::Control;
use anyhow::{Result, bail};
use std::cell::{Cell, RefCell};

struct Device {
    current: RefCell<Control>,
    writes: RefCell<Vec<bool>>,
    fail_next: Cell<bool>,
    replace_after_write: Cell<bool>,
    reads: Cell<usize>,
    change_at: Cell<Option<usize>>,
}
impl Device {
    fn new() -> Self {
        Self {
            current: RefCell::new(Control {
                endpoint: "render".into(),
                volume_part: "volume".into(),
                mute_part: "mute".into(),
                muted: true,
            }),
            writes: RefCell::new(Vec::new()),
            fail_next: Cell::new(false),
            replace_after_write: Cell::new(false),
            reads: Cell::new(0),
            change_at: Cell::new(None),
        }
    }
}
impl Io for Device {
    fn capture(&self) -> Result<Control> {
        let read = self.reads.get() + 1;
        self.reads.set(read);
        if self.change_at.get() == Some(read) {
            self.current.borrow_mut().muted = false;
        }
        Ok(self.current.borrow().clone())
    }
    fn write(&self, expected: &Control, muted: bool) -> Result<Control> {
        assert_eq!(&*self.current.borrow(), expected);
        self.writes.borrow_mut().push(muted);
        self.current.borrow_mut().muted = muted;
        if self.replace_after_write.replace(false) {
            self.current.borrow_mut().mute_part = "replacement".into();
            bail!("隔离写入后设备更换");
        }
        if self.fail_next.replace(false) {
            bail!("隔离部分写入失败");
        }
        Ok(self.current.borrow().clone())
    }
    fn ensure_available(&self) -> Result<()> {
        Ok(())
    }
}

#[test]
fn repeated_events_keep_the_first_baseline_and_restore_uses_a_fresh_preview() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store(dir.path().join("audio.json"));
    let io = Device::new();
    let initial = io.capture().unwrap();
    let muted = codec::change(&io, &store, &initial, false, &mut || Ok(())).unwrap();
    let stale = codec::preview(&io, &store).restore_revision.unwrap();
    let current = codec::change(&io, &store, &muted, true, &mut || Ok(())).unwrap();
    assert_eq!(store.load().unwrap().unwrap().initial, initial);
    let count = io.writes.borrow().len();
    assert!(codec::restore(&io, &store, &stale, &mut || Ok(())).is_err());
    assert_eq!(io.writes.borrow().len(), count);
    codec::change(&io, &store, &current, false, &mut || Ok(())).unwrap();
    let preview = codec::preview(&io, &store);
    codec::restore(&io, &store, &preview.restore_revision.unwrap(), &mut || {
        Ok(())
    })
    .unwrap();
    assert_eq!(io.capture().unwrap(), initial);
    assert!(store.load().unwrap().is_none());
}

#[test]
fn a_partial_write_is_compensated_and_a_failed_restore_keeps_a_retryable_baseline() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store(dir.path().join("audio.json"));
    let io = Device::new();
    let initial = io.capture().unwrap();
    io.fail_next.set(true);
    assert!(codec::change(&io, &store, &initial, false, &mut || Ok(())).is_err());
    assert_eq!(io.capture().unwrap(), initial);
    assert!(store.load().unwrap().is_some());
    let current = codec::change(&io, &store, &initial, false, &mut || Ok(())).unwrap();
    assert!(!current.muted);
    io.fail_next.set(true);
    let revision = codec::preview(&io, &store).restore_revision.unwrap();
    assert!(codec::restore(&io, &store, &revision, &mut || Ok(())).is_err());
    assert!(store.load().unwrap().is_some());
    let count = io.writes.borrow().len();
    let revision = codec::preview(&io, &store).restore_revision.unwrap();
    codec::restore(&io, &store, &revision, &mut || Ok(())).unwrap();
    assert_eq!(io.writes.borrow().len(), count);
}

#[test]
fn permission_or_state_changes_before_output_leave_no_new_hardware_ownership() {
    for changed_state in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let store = Store(dir.path().join("audio.json"));
        let io = Device::new();
        let initial = io.capture().unwrap();
        if changed_state {
            io.change_at.set(Some(3));
        }
        let mut checks = 0;
        let result = codec::change(&io, &store, &initial, false, &mut || {
            checks += 1;
            if !changed_state && checks == 2 {
                bail!("许可已撤销");
            }
            Ok(())
        });
        assert!(result.is_err());
        assert!(io.writes.borrow().is_empty());
        assert!(store.load().unwrap().is_none());
    }
}

#[test]
fn a_replaced_control_cannot_receive_an_old_target_or_another_devices_restore() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store(dir.path().join("audio.json"));
    let io = Device::new();
    codec::change(&io, &store, &io.capture().unwrap(), false, &mut || Ok(())).unwrap();
    let before = std::fs::read(&store.0).unwrap();
    io.current.borrow_mut().mute_part = "replacement".into();
    let count = io.writes.borrow().len();
    let expected = io.capture().unwrap();
    assert!(codec::change(&io, &store, &expected, true, &mut || Ok(())).is_err());
    let preview = codec::preview(&io, &store);
    assert!(preview.initial.is_some());
    assert!(preview.restore_revision.is_none());
    assert!(preview.restore_error.is_some());
    assert_eq!(io.writes.borrow().len(), count);
    assert_eq!(std::fs::read(&store.0).unwrap(), before);
}

#[test]
fn device_replacement_during_a_partial_write_never_receives_the_old_devices_compensation() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store(dir.path().join("audio.json"));
    let io = Device::new();
    let initial = io.capture().unwrap();
    io.replace_after_write.set(true);
    assert!(codec::change(&io, &store, &initial, false, &mut || Ok(())).is_err());
    assert_eq!(*io.writes.borrow(), [false]);
    assert_eq!(store.load().unwrap().unwrap().initial, initial);
    assert!(codec::preview(&io, &store).restore_revision.is_none());
}

#[test]
fn a_revoked_permission_after_output_compensates_without_requiring_the_old_permission() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store(dir.path().join("audio.json"));
    let io = Device::new();
    let initial = io.capture().unwrap();
    let mut checks = 0;
    let result = codec::change(&io, &store, &initial, false, &mut || {
        checks += 1;
        if checks == 3 {
            bail!("写入后许可已撤销");
        }
        Ok(())
    });
    assert!(result.is_err());
    assert_eq!(io.capture().unwrap(), initial);
    assert_eq!(*io.writes.borrow(), [false, true]);
    assert!(store.load().unwrap().is_some());
}

#[test]
fn malformed_records_and_noop_reads_do_not_fabricate_or_initialize_recovery() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store(dir.path().join("audio.json"));
    let io = Device::new();
    let profile = codec::preview(&io, &store);
    assert!(profile.initial.is_none());
    assert!(!store.0.exists());
    codec::change(&io, &store, &io.capture().unwrap(), true, &mut || Ok(())).unwrap();
    assert!(!store.0.exists());
    for bytes in [
        b"bad".as_slice(),
        br#"{"version":1}"#,
        br#"{"version":0,"initial":{"endpoint":"a","volumePart":"b","mutePart":"c","muted":true}}"#,
        br#"{"version":1,"initial":{"endpoint":"","volumePart":"b","mutePart":"c","muted":true}}"#,
        br#"{"version":1,"initial":{"endpoint":"a","volumePart":"b","mutePart":"c","muted":true,"old":0}}"#,
    ] {
        std::fs::write(&store.0, bytes).unwrap();
        let profile = codec::preview(&io, &store);
        assert!(profile.restore_error.is_some());
        assert!(codec::change(&io, &store, &io.capture().unwrap(), false, &mut || Ok(())).is_err());
        assert!(io.writes.borrow().is_empty());
        assert_eq!(std::fs::read(&store.0).unwrap(), bytes);
    }
}
