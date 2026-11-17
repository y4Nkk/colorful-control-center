use super::{
    model::*,
    store::{self, Record},
    transaction::{self, Io, Plan},
};
use anyhow::{Result, bail};
use std::{cell::RefCell, collections::BTreeMap};
#[path = "controller_tests.rs"]
mod controller_tests;
#[path = "recovery_tests.rs"]
mod recovery_tests;

fn device() -> Device {
    let mut values = FIELDS
        .into_iter()
        .map(|field| (field, None))
        .collect::<BTreeMap<_, _>>();
    values.insert(Field::PowerSavingMode, Some(Value::string("0")));
    values.insert(Field::WakeOnSlot, Some(Value::string("1")));
    Device {
        identity: Identity {
            instance: "PCI\\VEN_ABCD&DEV_1234\\1".into(),
            driver: format!("{CLASS}\\0001"),
            net_cfg_id: Some(Value::string("isolated-id")),
            driver_version: Some(Value::string("1.0")),
            matching_id: None,
        },
        name: "隔离网卡".into(),
        values,
    }
}
fn frame(device: &Device) -> Frame {
    Frame {
        capabilities: Capabilities {
            power_saving: true,
            wake_patch: true,
        },
        recommendation: Some(true),
        devices: vec![device.clone()],
    }
}
fn request(device: &Device) -> Request {
    Request {
        expected_revision: "preview".into(),
        device_id: device.identity.instance.clone(),
        power_saving: Some(true),
        apply_wake_patch: true,
    }
}
fn plan(device: &Device) -> Plan {
    Plan {
        expected: device.clone(),
        changes: changes(&frame(device), &request(device)).unwrap(),
    }
}
struct State {
    device: Device,
    writes: usize,
    failures: Vec<usize>,
    refused: Vec<usize>,
    external: bool,
    ignored: bool,
}
struct Memory(RefCell<State>);
impl Memory {
    fn new() -> Self {
        Self(RefCell::new(State {
            device: device(),
            writes: 0,
            failures: vec![],
            refused: vec![],
            external: false,
            ignored: false,
        }))
    }
    fn current(&self) -> Device {
        self.0.borrow().device.clone()
    }
}
impl Io for Memory {
    fn inspect(&self, _: &str) -> Result<Device> {
        Ok(self.current())
    }
    fn prepare(&self, _: &Device, _: &Change) -> Result<()> {
        Ok(())
    }
    fn write(&self, expected: &Identity, field: Field, value: &Option<Value>) -> Result<()> {
        let mut state = self.0.borrow_mut();
        if state.device.identity != *expected {
            bail!("隔离身份变化");
        }
        state.writes += 1;
        if state.refused.contains(&state.writes) {
            bail!("隔离写入被拒绝");
        }
        if !state.ignored {
            state.device.values.insert(field, value.clone());
        }
        if state.external && state.writes == 1 {
            state
                .device
                .values
                .insert(Field::WakeOnLink, Some(Value::string("external")));
        }
        if state.failures.contains(&state.writes) {
            bail!("隔离部分写入失败");
        }
        Ok(())
    }
}

#[test]
fn original_lan_patch_has_three_fixed_targets_and_preserves_sentinel_values() {
    let io = Memory::new();
    let before = io.current();
    let plan = plan(&before);
    assert_eq!(plan.changes.len(), 3);
    transaction::execute(&io, &plan, &mut Default::default()).unwrap();
    let after = io.current();
    assert_eq!(after.power_saving(), Some(true));
    assert_eq!(
        after.values[&Field::PnpCapabilities],
        Some(Value::dword(256))
    );
    assert_eq!(after.values[&Field::WakeOnLink], Some(Value::string("2")));
    assert_eq!(
        after.values[&Field::WakeOnSlot],
        before.values[&Field::WakeOnSlot]
    );
    assert_eq!(after.values[&Field::S5WakeOnLan], None);
}
#[test]
fn s5_and_shutdown_variants_do_not_create_wake_on_link() {
    for sentinel in [Field::S5WakeOnLan, Field::ShutdownWake] {
        let mut device = device();
        device.values.insert(Field::WakeOnSlot, None);
        device.values.insert(sentinel, Some(Value::string("0")));
        let mut request = request(&device);
        request.power_saving = None;
        let changes = changes(&frame(&device), &request).unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].field, Field::PnpCapabilities);
    }
}
#[test]
fn raw_types_malformed_strings_and_unknown_power_state_are_not_guessed() {
    for value in [
        Value::dword(1),
        Value::string("unexpected"),
        Value {
            kind: 1,
            bytes: vec![49, 0],
        },
        Value {
            kind: 1,
            bytes: vec![49, 0, 0],
        },
        Value {
            kind: 1,
            bytes: vec![49, 0, 0, 0, 0, 0],
        },
    ] {
        let mut device = device();
        device.values.insert(Field::PowerSavingMode, Some(value));
        assert_eq!(device.power_saving(), None);
        assert!(changes(&frame(&device), &request(&device)).is_err());
    }
}
#[test]
fn every_partial_write_failure_compensates_including_new_value_deletion() {
    for failure in 1..=3 {
        let io = Memory::new();
        let before = io.current();
        io.0.borrow_mut().failures = vec![failure];
        assert!(transaction::execute(&io, &plan(&before), &mut Default::default()).is_err());
        assert_eq!(io.current(), before);
    }
}
#[test]
fn ignored_write_is_reported_by_readback_and_compensation_is_attempted() {
    let io = Memory::new();
    let before = io.current();
    io.0.borrow_mut().ignored = true;
    assert!(
        transaction::execute(&io, &plan(&before), &mut Default::default())
            .unwrap_err()
            .to_string()
            .contains("回读")
    );
    assert_eq!(io.0.borrow().writes, 2);
    assert_eq!(io.current(), before);
}
#[test]
fn stale_snapshot_and_driver_replacement_fail_before_any_mutation() {
    let io = Memory::new();
    let plan = plan(&io.current());
    io.0.borrow_mut().device.identity.driver_version = Some(Value::string("2.0"));
    assert!(transaction::execute(&io, &plan, &mut Default::default()).is_err());
    assert_eq!(io.0.borrow().writes, 0);
    let mut record = Record::default();
    record.remember(&plan).unwrap();
    assert!(record.restore_plan(&io.current()).is_err());
    assert!(record.remember(&self::plan(&io.current())).is_err());
}
#[test]
fn unattempted_external_field_change_is_preserved_during_compensation() {
    let io = Memory::new();
    let plan = plan(&io.current());
    io.0.borrow_mut().external = true;
    assert!(transaction::execute(&io, &plan, &mut Default::default()).is_err());
    assert_eq!(io.current().power_saving(), Some(false));
    assert_eq!(io.current().values[&Field::PnpCapabilities], None);
    assert_eq!(
        io.current().values[&Field::WakeOnLink],
        Some(Value::string("external"))
    );
}
#[test]
fn restore_deletes_only_created_values_and_keeps_unowned_latest_fields() {
    let io = Memory::new();
    let before = io.current();
    let mut request = request(&before);
    request.power_saving = None;
    let plan = Plan {
        expected: before.clone(),
        changes: changes(&frame(&before), &request).unwrap(),
    };
    let mut record = Record::default();
    record.remember(&plan).unwrap();
    transaction::execute(&io, &plan, &mut Default::default()).unwrap();
    io.0.borrow_mut()
        .device
        .values
        .insert(Field::PowerSavingMode, Some(Value::string("1")));
    let restore = record.restore_plan(&io.current()).unwrap().unwrap();
    transaction::restore(&io, &restore, &mut Default::default()).unwrap();
    assert_eq!(io.current().power_saving(), Some(true));
    assert_eq!(io.current().values[&Field::PnpCapabilities], None);
    assert_eq!(io.current().values[&Field::WakeOnLink], None);
    assert!(record.restore_plan(&io.current()).unwrap().is_none());
}
#[test]
fn compensation_failure_keeps_record_for_independent_restore_and_retry() {
    let io = Memory::new();
    let plan = plan(&io.current());
    let mut record = Record::default();
    record.remember(&plan).unwrap();
    io.0.borrow_mut().failures = vec![2];
    io.0.borrow_mut().refused = vec![3];
    assert!(
        transaction::execute(&io, &plan, &mut Default::default())
            .unwrap_err()
            .to_string()
            .contains("补偿失败")
    );
    let restore = record.restore_plan(&io.current()).unwrap().unwrap();
    transaction::restore(&io, &restore, &mut Default::default()).unwrap();
    assert_eq!(io.current(), device());
    assert_eq!(record.devices.len(), 1);
}
#[test]
fn restore_attempts_other_initial_fields_even_if_one_fails() {
    let io = Memory::new();
    let plan = plan(&io.current());
    let mut record = Record::default();
    record.remember(&plan).unwrap();
    transaction::execute(&io, &plan, &mut Default::default()).unwrap();
    let restore = record.restore_plan(&io.current()).unwrap().unwrap();
    io.0.borrow_mut().failures = vec![4];
    assert!(transaction::restore(&io, &restore, &mut Default::default()).is_err());
    assert_eq!(io.0.borrow().writes, 6);
    assert_eq!(io.current(), device());
}
#[test]
fn fixed_write_set_rejects_unknown_duplicate_and_mismatched_fields() {
    let original = plan(&device());
    let mut bad = original.clone();
    bad.changes[0].field = Field::WakeOnSlot;
    assert!(bad.validate().is_err());
    let mut bad = original.clone();
    bad.changes.push(bad.changes[0].clone());
    assert!(bad.validate().is_err());
    let mut bad = original.clone();
    bad.changes[0].before = None;
    assert!(bad.validate().is_err());
    let mut bad = original;
    bad.expected.identity.driver = format!("{CLASS}\\..\\other");
    assert!(bad.validate().is_err());
    assert!(serde_json::from_str::<Request>(r#"{"expectedRevision":"x","deviceId":"x","powerSaving":true,"applyWakePatch":false,"path":"x"}"#).is_err());
}
#[test]
fn capability_flags_are_distinct_and_unsupported_targets_are_rejected() {
    let mut bytes = [0; 24];
    bytes[0] = 1;
    bytes[23] = 3;
    assert!(!Capabilities::decode(&bytes).unwrap().any());
    bytes[21] = 128;
    bytes[6] = 32;
    assert!(Capabilities::decode(&bytes).unwrap().power_saving);
    assert!(Capabilities::decode(&bytes).unwrap().wake_patch);
    bytes[0] = 2;
    assert!(Capabilities::decode(&bytes).is_err());
    let device = device();
    let mut frame = frame(&device);
    frame.capabilities = Capabilities::default();
    assert!(changes(&frame, &request(&device)).is_err());
    frame.devices.push(device);
    assert!(frame.validate().is_err());
}
#[test]
fn recovery_file_survives_reopen_preserves_raw_values_and_first_baseline() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("lan-recovery.json");
    let mut original = device();
    original.values.insert(
        Field::PnpCapabilities,
        Some(Value {
            kind: 3,
            bytes: vec![0, 255, 3],
        }),
    );
    let first = plan(&original);
    let empty = store::read(&path).unwrap();
    let mut record = empty.clone();
    record.remember(&first).unwrap();
    store::write(&path, &empty, &record).unwrap();
    assert_eq!(store::read(&path).unwrap(), record);
    let mut later = original.clone();
    later
        .values
        .insert(Field::PowerSavingMode, Some(Value::string("1")));
    let mut request = request(&later);
    request.power_saving = Some(false);
    request.apply_wake_patch = false;
    record
        .remember(&Plan {
            expected: later.clone(),
            changes: changes(&frame(&later), &request).unwrap(),
        })
        .unwrap();
    assert_eq!(
        record.devices[0].fields[&Field::PowerSavingMode],
        Some(Value::string("0"))
    );
    assert_eq!(
        record.devices[0].fields[&Field::PnpCapabilities],
        original.values[&Field::PnpCapabilities]
    );
}
#[test]
fn corrupt_external_or_readonly_recovery_records_are_never_overwritten() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("lan-recovery.json");
    let mut record = Record::default();
    record.remember(&plan(&device())).unwrap();
    std::fs::write(&path, b"broken").unwrap();
    assert!(store::write(&path, &Record::default(), &record).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"broken");
    let mut invalid = record.clone();
    invalid.version = 2;
    assert!(invalid.validate().is_err());
    let mut invalid = record.clone();
    invalid.devices[0].fields.insert(Field::WakeOnSlot, None);
    assert!(invalid.validate().is_err());
    std::fs::remove_file(&path).unwrap();
    store::write(&path, &Record::default(), &record).unwrap();
    assert!(store::write(&path, &Record::default(), &record).is_err());
}
