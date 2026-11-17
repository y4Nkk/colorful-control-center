use super::*;
use crate::platform::lan::{
    recovery::Prepared,
    worker::{Invocation, Operation},
};
struct JournalRequired<'a> {
    io: &'a Memory,
    path: &'a std::path::Path,
}
impl Io for JournalRequired<'_> {
    fn inspect(&self, instance: &str) -> Result<Device> {
        self.io.inspect(instance)
    }
    fn prepare(&self, _: &Device, _: &Change) -> Result<()> {
        bail!("测试必须使用真实恢复协调")
    }
    fn write(&self, identity: &Identity, field: Field, value: &Option<Value>) -> Result<()> {
        let record = store::read(self.path)?;
        if !record
            .devices
            .iter()
            .any(|device| device.identity == *identity && device.fields.contains_key(&field))
        {
            bail!("字段尚未持久记录，禁止写入");
        }
        self.io.write(identity, field, value)
    }
}
#[test]
fn every_actual_attempt_is_persisted_first_and_unattempted_fields_are_not_owned() {
    for failure in 1..=3 {
        let io = Memory::new();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("lan-recovery.json");
        let guarded = JournalRequired {
            io: &io,
            path: &path,
        };
        let prepared = Prepared {
            io: &guarded,
            path: &path,
            record: &RefCell::new(Record::default()),
        };
        let plan = plan(&io.current());
        io.0.borrow_mut().failures = vec![failure];
        assert!(transaction::execute(&prepared, &plan, &mut Default::default()).is_err());
        let record = store::read(&path).unwrap();
        assert_eq!(record.devices[0].fields.len(), failure);
        assert_eq!(io.current(), device());
        assert_eq!(
            record.devices[0].fields[&Field::PowerSavingMode],
            Some(Value::string("0"))
        );
    }
}
#[test]
fn first_persistence_failure_prevents_any_system_mutation() {
    let io = Memory::new();
    let directory = tempfile::tempdir().unwrap();
    let parent = directory.path().join("not-a-directory");
    std::fs::write(&parent, b"keep").unwrap();
    let path = parent.join("lan-recovery.json");
    let prepared = Prepared {
        io: &io,
        path: &path,
        record: &RefCell::new(Record::default()),
    };
    assert!(
        transaction::execute(&prepared, &plan(&io.current()), &mut Default::default()).is_err()
    );
    assert_eq!(io.0.borrow().writes, 0);
    assert_eq!(std::fs::read(parent).unwrap(), b"keep");
}
#[test]
fn changed_recovery_file_prevents_writes_and_preserves_external_contents() {
    let io = Memory::new();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("lan-recovery.json");
    let mut external = Record::default();
    external.last_error = Some("外部记录".into());
    store::write(&path, &Record::default(), &external).unwrap();
    let prepared = Prepared {
        io: &io,
        path: &path,
        record: &RefCell::new(Record::default()),
    };
    assert!(
        transaction::execute(&prepared, &plan(&io.current()), &mut Default::default()).is_err()
    );
    assert_eq!(io.0.borrow().writes, 0);
    assert_eq!(store::read(&path).unwrap(), external);
}
#[test]
fn reopen_restores_created_values_and_last_error_is_explicitly_cleared() {
    let io = Memory::new();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("lan-recovery.json");
    {
        let prepared = Prepared {
            io: &io,
            path: &path,
            record: &RefCell::new(Record::default()),
        };
        transaction::execute(&prepared, &plan(&io.current()), &mut Default::default()).unwrap();
        prepared.report(Some("隔离后续失败".into())).unwrap();
    }
    let record = store::read(&path).unwrap();
    assert_eq!(record.last_error.as_deref(), Some("隔离后续失败"));
    let restore = record.restore_plan(&io.current()).unwrap().unwrap();
    let prepared = Prepared {
        io: &io,
        path: &path,
        record: &RefCell::new(record),
    };
    transaction::restore(&prepared, &restore, &mut Default::default()).unwrap();
    prepared.report(None).unwrap();
    assert_eq!(io.current(), device());
    assert!(store::read(&path).unwrap().last_error.is_none());
}
#[test]
fn review_digest_binds_capabilities_raw_fields_and_recovery_records() {
    let original = frame(&device());
    let record = Record::default();
    let revision = digest(&(&original, &record)).unwrap();
    let mut changed = original.clone();
    changed.capabilities.wake_patch = false;
    assert_ne!(revision, digest(&(&changed, &record)).unwrap());
    let mut changed = original.clone();
    changed.devices[0]
        .values
        .insert(Field::PnpCapabilities, Some(Value::dword(9)));
    assert_ne!(revision, digest(&(&changed, &record)).unwrap());
    let mut changed = record;
    changed.remember(&plan(&device())).unwrap();
    assert_ne!(revision, digest(&(&original, &changed)).unwrap());
}
#[test]
fn worker_arguments_round_trip_unicode_as_inert_hex_and_bind_the_request_file() {
    use crate::platform::lan::{
        authorization::Binding,
        worker::{Intent, hash_bytes},
    };
    let root = tempfile::Builder::new()
        .prefix("隔离 目录")
        .tempdir()
        .unwrap();
    let binding = Binding {
        parent_id: 1,
        token: "0123456789abcdef0123456789abcdef".into(),
    };
    let directory = tempfile::Builder::new()
        .prefix(&format!(".lan-session-{}-", binding.token))
        .tempdir_in(root.path())
        .unwrap();
    let operation = Operation {
        intent: Intent::Apply {
            request: request(&device()),
        },
        recovery_file: root.path().join("lan-recovery.json"),
    };
    let bytes = operation.encoded().unwrap();
    std::fs::write(directory.path().join("request.json"), &bytes).unwrap();
    let invocation = Invocation {
        session_dir: directory.path().into(),
        request_hash: hash_bytes(&bytes),
        authorization: binding,
    };
    let arguments = invocation.arguments().unwrap();
    let decoded =
        Invocation::from_args(&arguments.split(' ').map(str::to_owned).collect::<Vec<_>>())
            .unwrap();
    assert_eq!(
        decoded.operation().unwrap().recovery_file,
        operation.recovery_file
    );
    assert!(Invocation::from_args(&["--oem-lan-policy".into(), "zz".into()]).is_err());
    std::fs::write(directory.path().join("request.json"), b"{}").unwrap();
    assert!(decoded.operation().is_err());
    let mut invalid = serde_json::to_value(&operation).unwrap();
    invalid["restoring"] = serde_json::json!(true);
    assert!(serde_json::from_value::<Operation>(invalid).is_err());
}

#[test]
fn record_writer_never_publishes_a_size_that_the_reader_would_reject() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("lan-recovery.json");
    let empty = Record::default();
    let mut oversized = empty.clone();
    for index in 0..128 {
        let mut identity = device().identity;
        identity.instance = format!("PCI\\ISOLATED\\{index}");
        identity.driver = format!("{CLASS}\\{index:04}");
        oversized.devices.push(store::Initial {
            identity,
            fields: [
                Field::PowerSavingMode,
                Field::PnpCapabilities,
                Field::WakeOnLink,
            ]
            .into_iter()
            .map(|field| {
                (
                    field,
                    Some(Value {
                        kind: 3,
                        bytes: vec![255; 4096],
                    }),
                )
            })
            .collect(),
        });
    }
    assert!(store::write(&path, &empty, &oversized).is_err());
    assert!(!path.exists());
}
