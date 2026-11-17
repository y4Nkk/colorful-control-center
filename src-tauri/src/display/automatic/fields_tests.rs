use super::fields::{FileStore, Io, Record, change, restore};
use anyhow::{Result, bail};
use std::{cell::RefCell, collections::BTreeSet};

struct Memory(RefCell<Data>);
struct Data {
    identity: String,
    fields: [u8; 3],
    writes: Vec<(usize, u8)>,
    fail_after: BTreeSet<usize>,
}
impl Io for Memory {
    fn identity(&self) -> Result<String> {
        Ok(self.0.borrow().identity.clone())
    }
    fn fields(&self) -> Result<[u8; 3]> {
        Ok(self.0.borrow().fields)
    }
    fn write(&self, index: usize, value: u8) -> Result<()> {
        let mut state = self.0.borrow_mut();
        state.writes.push((index, value));
        state.fields[index] = value;
        if state.fail_after.contains(&state.writes.len()) {
            bail!("隔离写后故障");
        }
        Ok(())
    }
}
fn setup() -> (tempfile::TempDir, FileStore, Record, Memory) {
    let dir = tempfile::tempdir().unwrap();
    let store = FileStore(dir.path().join("refresh.json"));
    let identity = "a".repeat(64);
    let initial_display = super::facts_tests::setup().0.borrow().1[0].current.clone();
    let record = Record::new(identity.clone(), [0, 0, 144], initial_display).unwrap();
    store.save(None, &record).unwrap();
    let io = Memory(RefCell::new(Data {
        identity,
        fields: [0, 0, 144],
        writes: vec![],
        fail_after: BTreeSet::new(),
    }));
    (dir, store, record, io)
}

#[test]
fn repeated_changes_preserve_first_values_and_ignore_unowned_fields_on_restore() {
    let (_dir, store, mut record, io) = setup();
    change(&io, &store, &mut record, 1, 1, &mut || Ok(())).unwrap();
    change(&io, &store, &mut record, 2, 60, &mut || Ok(())).unwrap();
    change(&io, &store, &mut record, 2, 120, &mut || Ok(())).unwrap();
    io.0.borrow_mut().fields[0] = 1;
    restore(&io, &store, &mut record, &mut || Ok(())).unwrap();
    assert_eq!(io.fields().unwrap(), [1, 0, 144]);
    assert!(store.load().unwrap().is_none());
}

#[test]
fn stale_external_values_and_revoked_permission_stop_before_writing() {
    let (_dir, store, mut record, io) = setup();
    io.0.borrow_mut().fields[2] = 120;
    assert!(change(&io, &store, &mut record, 1, 1, &mut || Ok(())).is_err());
    io.0.borrow_mut().fields[2] = 144;
    assert!(change(&io, &store, &mut record, 1, 1, &mut || bail!("取消")).is_err());
    io.0.borrow_mut().identity = "b".repeat(64);
    assert!(change(&io, &store, &mut record, 1, 1, &mut || Ok(())).is_err());
    assert!(io.0.borrow().writes.is_empty());
}

#[test]
fn write_after_failure_compensates_and_failed_compensation_retains_recovery() {
    let (_dir, store, mut record, io) = setup();
    io.0.borrow_mut().fail_after.extend([1, 2]);
    assert!(change(&io, &store, &mut record, 1, 1, &mut || Ok(())).is_err());
    let mut recovered = store.load().unwrap().unwrap();
    assert!(serde_json::to_value(&recovered).unwrap()["attempt"].is_object());
    io.0.borrow_mut().fail_after.clear();
    restore(&io, &store, &mut recovered, &mut || Ok(())).unwrap();
    assert_eq!(io.fields().unwrap(), [0, 0, 144]);
    assert!(store.load().unwrap().is_none());
}

#[test]
fn restore_preserves_external_owned_changes_and_allows_retry() {
    let (_dir, store, mut record, io) = setup();
    change(&io, &store, &mut record, 2, 60, &mut || Ok(())).unwrap();
    io.0.borrow_mut().fields[2] = 120;
    assert!(restore(&io, &store, &mut record, &mut || Ok(())).is_err());
    assert_eq!(io.fields().unwrap()[2], 120);
    assert!(store.load().unwrap().is_some());
    io.0.borrow_mut().fields[2] = 60;
    restore(&io, &store, &mut record, &mut || Ok(())).unwrap();
    assert_eq!(io.fields().unwrap()[2], 144);
}

#[test]
fn strict_record_validation_rejects_unknown_fields_and_bad_values() {
    let (_dir, store, record, _io) = setup();
    let display = super::facts_tests::setup().0.borrow().1[0].current.clone();
    assert!(Record::new("bad".into(), [0, 0, 0], display.clone()).is_err());
    assert!(Record::new("a".repeat(64), [2, 0, 0], display).is_err());
    let mut value = serde_json::to_value(record).unwrap();
    value["legacy"] = true.into();
    std::fs::write(&store.0, serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(store.load().is_err());
}
