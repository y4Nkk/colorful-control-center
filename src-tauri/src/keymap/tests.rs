use super::*;
use ccc_core::keymap::{Action, Binding, DeviceSettings};
use model::{Phase, Record};
use std::cell::{Cell, RefCell};
use store::{Loaded, Store};
#[path = "tests/scope_tests.rs"]
mod scope_tests;
#[path = "tests/storage_tests.rs"]
mod storage_tests;

fn original_config() -> Configuration {
    Configuration {
        devices: vec![
            DeviceSettings {
                device: Device::Keyboard,
                enabled: false,
                bindings: vec![Binding {
                    trigger: 30,
                    action: Action::Disable,
                }],
            },
            DeviceSettings {
                device: Device::Mouse,
                enabled: true,
                bindings: vec![Binding {
                    trigger: 4,
                    action: Action::Disable,
                }],
            },
        ],
    }
}
fn target(device: Device) -> Configuration {
    Configuration {
        devices: vec![DeviceSettings {
            device,
            enabled: true,
            bindings: vec![Binding {
                trigger: if device == Device::Keyboard { 31 } else { 16 },
                action: Action::Disable,
            }],
        }],
    }
}
struct Memory {
    actual: RefCell<Configuration>,
    flags: Cell<[u8; 2]>,
    source: RefCell<Option<original::Source>>,
    capability: Cell<[u8; 15]>,
    effects: RefCell<Vec<String>>,
    fail_replace: Cell<bool>,
    fail_flags: Cell<bool>,
    fail_programs: Cell<bool>,
    source_error: RefCell<Option<String>>,
    fail_all: Cell<bool>,
    availability_checks: Cell<usize>,
    block_at: Cell<usize>,
}
impl Memory {
    fn new() -> Self {
        let mut capability = [0; 15];
        capability[0] = 1;
        capability[13] = 2;
        Self {
            actual: RefCell::new(original_config()),
            flags: Cell::new([0, 1]),
            source: RefCell::new(Some(original::Source {
                name: "原厂活动配置".into(),
                configuration: original_config(),
                fingerprint: "source-1".into(),
            })),
            capability: Cell::new(capability),
            effects: Default::default(),
            fail_replace: Cell::new(false),
            fail_flags: Cell::new(false),
            fail_programs: Cell::new(false),
            source_error: RefCell::new(None),
            fail_all: Cell::new(false),
            availability_checks: Cell::new(0),
            block_at: Cell::new(usize::MAX),
        }
    }
    fn review(&self, store: &impl Store) -> String {
        read(self, store).unwrap().unwrap().revision
    }
}
impl Io for Memory {
    fn facts(&self) -> Result<Facts> {
        Ok(Facts {
            capability: self.capability.get(),
            flags: self.flags.get(),
            original: self.source.borrow().clone(),
            original_error: self.source_error.borrow().clone(),
        })
    }
    fn ensure_available(&self) -> Result<()> {
        let count = self.availability_checks.get() + 1;
        self.availability_checks.set(count);
        if count >= self.block_at.get() {
            bail!("隔离原厂控制者仍在运行");
        }
        Ok(())
    }
    fn replace(&self, configuration: &Configuration) -> Result<()> {
        self.effects.borrow_mut().push("mapping".into());
        let next = model::merge(&self.actual.borrow(), configuration)?;
        *self.actual.borrow_mut() = next;
        if self.fail_replace.replace(false) || self.fail_all.get() {
            bail!("隔离映射部分失败");
        }
        Ok(())
    }
    fn save_flags(&self, flags: [u8; 2], scope: &[Device]) -> Result<()> {
        self.effects.borrow_mut().push("flags".into());
        let mut current = self.flags.get();
        for device in scope {
            current[model::index(*device)] = flags[model::index(*device)];
        }
        self.flags.set(current);
        if self.fail_flags.replace(false) || self.fail_all.get() {
            bail!("隔离启用保存部分失败");
        }
        Ok(())
    }
    fn prepare_programs(&self, _: &Record, _: Arc<Mutex<()>>) -> Result<Option<launch::Session>> {
        if self.fail_programs.replace(false) {
            bail!("隔离程序监听准备失败");
        }
        Ok(None)
    }
}
#[derive(Default)]
struct MemoryStore {
    record: RefCell<Option<Record>>,
    revision: Cell<usize>,
    writes: Cell<usize>,
    fail_at: Cell<usize>,
}
impl MemoryStore {
    fn new() -> Self {
        Self {
            fail_at: Cell::new(usize::MAX),
            ..Default::default()
        }
    }
}
impl Store for MemoryStore {
    fn load(&self) -> Result<Loaded> {
        Ok(Loaded {
            record: self.record.borrow().clone(),
            revision: self.revision.get().to_string(),
        })
    }
    fn replace(&self, expected: &Loaded, record: &Record) -> Result<Loaded> {
        record.validate()?;
        if expected.revision != self.revision.get().to_string() {
            bail!("隔离记录外部变更");
        }
        let writes = self.writes.get() + 1;
        self.writes.set(writes);
        if writes == self.fail_at.get() {
            bail!("隔离记录保存失败");
        }
        *self.record.borrow_mut() = Some(record.clone());
        self.revision.set(self.revision.get() + 1);
        self.load()
    }
}
fn apply(io: &Memory, store: &MemoryStore, configuration: &Configuration) -> Result<()> {
    let revision = io.review(store);
    let frame = capture(io, store)?;
    frame.review(&revision, false)?;
    run(
        io,
        store,
        transaction::prepare(
            frame,
            &Target {
                configuration: configuration.clone(),
                programs_enabled: true,
            },
        )?,
    )
}
fn restore(io: &Memory, store: &MemoryStore) -> Result<()> {
    let revision = io.review(store);
    let frame = capture(io, store)?;
    frame.review(&revision, true)?;
    run(io, store, transaction::initial(frame)?)
}
fn run(io: &Memory, store: &MemoryStore, plan: Plan) -> Result<()> {
    let candidate = io.prepare_programs(&plan.record, Arc::new(Mutex::new(())))?;
    let mut pending = None;
    let result = transaction::execute(io, store, &plan, &mut pending).and_then(|_| {
        transaction::commit(io, store, &plan, pending.as_mut().unwrap())?;
        if let Some(session) = &candidate {
            session.activate()?;
        }
        Ok(())
    });
    if let Err(error) = result {
        let recovery = pending.as_ref().map_or(Ok(()), |pending| {
            transaction::rollback(io, store, &plan, pending)
        });
        bail!(ccc_core::msr::with_recovery(
            format!("{error:#}"),
            recovery.map_err(|e| format!("{e:#}"))
        ));
    }
    Ok(())
}
#[test]
fn explicit_device_changes_keep_other_mappings_and_restore_the_first_full_baseline() {
    let io = Memory::new();
    let store = MemoryStore::new();
    apply(&io, &store, &target(Device::Keyboard)).unwrap();
    assert_eq!(io.actual.borrow().devices[1], original_config().devices[1]);
    assert_eq!(io.flags.get(), [1, 1]);
    apply(&io, &store, &target(Device::Mouse)).unwrap();
    let record = store.record.borrow().clone().unwrap();
    assert_eq!(record.baseline.configuration, original_config());
    assert_eq!(record.owned.len(), 2);
    restore(&io, &store).unwrap();
    assert_eq!(*io.actual.borrow(), original_config());
    assert_eq!(io.flags.get(), [0, 1]);
}
#[test]
fn absent_original_and_invalid_targets_never_write_a_record_or_filter() {
    let io = Memory::new();
    let store = MemoryStore::new();
    io.source.borrow_mut().take();
    assert!(apply(&io, &store, &target(Device::Keyboard)).is_err());
    assert!(store.record.borrow().is_none());
    assert!(io.effects.borrow().is_empty());
    let io = Memory::new();
    let mut invalid = target(Device::Keyboard);
    invalid.devices[0].bindings[0].trigger = 0;
    assert!(apply(&io, &store, &invalid).is_err());
    assert!(io.effects.borrow().is_empty());
    assert!(store.record.borrow().is_none());
}
#[test]
fn saved_intent_precedes_every_mapping_write_and_unwritable_store_does_not_touch_filters() {
    let io = Memory::new();
    let store = MemoryStore::new();
    store.fail_at.set(1);
    assert!(apply(&io, &store, &target(Device::Keyboard)).is_err());
    assert!(io.effects.borrow().is_empty());
}
#[test]
fn mapping_flags_and_commit_partial_failures_compensate_and_keep_a_recovery_baseline() {
    for stage in 0..3 {
        let io = Memory::new();
        let store = MemoryStore::new();
        match stage {
            0 => io.fail_replace.set(true),
            1 => io.fail_flags.set(true),
            _ => store.fail_at.set(2),
        }
        assert!(apply(&io, &store, &target(Device::Keyboard)).is_err());
        assert_eq!(*io.actual.borrow(), original_config());
        assert_eq!(io.flags.get(), [0, 1]);
        assert_eq!(
            store.record.borrow().as_ref().unwrap().phase,
            Phase::Applied
        );
        assert!(read(&io, &store).unwrap().unwrap().can_restore);
    }
}
#[test]
fn failed_compensation_remains_pending_and_only_explicit_restore_can_recover() {
    let io = Memory::new();
    let store = MemoryStore::new();
    io.fail_all.set(true);
    let error = apply(&io, &store, &target(Device::Keyboard))
        .unwrap_err()
        .to_string();
    assert!(error.contains("恢复"));
    assert_eq!(
        store.record.borrow().as_ref().unwrap().phase,
        Phase::Prepared
    );
    io.fail_all.set(false);
    assert!(apply(&io, &store, &target(Device::Keyboard)).is_err());
    restore(&io, &store).unwrap();
    assert_eq!(*io.actual.borrow(), original_config());
}
#[test]
fn source_and_record_changes_invalidate_review_but_saved_recovery_survives_original_file_removal() {
    let io = Memory::new();
    let store = MemoryStore::new();
    let old = io.review(&store);
    io.source.borrow_mut().as_mut().unwrap().fingerprint = "changed".into();
    assert!(capture(&io, &store).unwrap().review(&old, false).is_err());
    apply(&io, &store, &target(Device::Keyboard)).unwrap();
    io.source.borrow_mut().take();
    restore(&io, &store).unwrap();
    assert_eq!(*io.actual.borrow(), original_config());
}
#[test]
fn damaged_original_source_invalidates_review_but_keeps_the_persisted_recovery_readable() {
    let io = Memory::new();
    let store = MemoryStore::new();
    apply(&io, &store, &target(Device::Keyboard)).unwrap();
    let previous = io.review(&store);
    io.source.borrow_mut().take();
    *io.source_error.borrow_mut() = Some("原厂XML损坏 hash=fixture".into());
    assert!(
        capture(&io, &store)
            .unwrap()
            .review(&previous, false)
            .is_err()
    );
    let profile = read(&io, &store).unwrap().unwrap();
    assert!(profile.can_restore);
    assert!(
        profile
            .warnings
            .iter()
            .any(|warning| warning.contains("XML损坏"))
    );
    restore(&io, &store).unwrap();
    assert_eq!(*io.actual.borrow(), original_config());
}
#[test]
fn lost_capability_preserves_the_record_and_does_not_report_fake_enable_flags() {
    let io = Memory::new();
    let store = MemoryStore::new();
    apply(&io, &store, &target(Device::Keyboard)).unwrap();
    let before = store.load().unwrap().revision;
    let capability = io.capability.replace([0; 15]);
    assert!(
        read(&io, &store)
            .err()
            .unwrap()
            .to_string()
            .contains("原恢复记录已保留")
    );
    assert_eq!(before, store.load().unwrap().revision);
    io.capability.set(capability);
    restore(&io, &store).unwrap();
    assert_eq!(*io.actual.borrow(), original_config());
}
#[test]
fn an_owner_appearing_after_review_never_triggers_compensation_writes() {
    let io = Memory::new();
    let store = MemoryStore::new();
    let frame = capture(&io, &store).unwrap();
    let revision = frame.revision().unwrap();
    frame.review(&revision, false).unwrap();
    io.block_at.set(2);
    let plan = transaction::prepare(
        frame,
        &Target {
            configuration: target(Device::Keyboard),
            programs_enabled: true,
        },
    )
    .unwrap();
    assert!(run(&io, &store, plan).is_err());
    assert!(io.effects.borrow().is_empty());
    assert_eq!(
        store.record.borrow().as_ref().unwrap().phase,
        Phase::Applied
    );
}
#[test]
fn unknown_capabilities_and_bad_enable_flags_are_never_silently_accepted() {
    let io = Memory::new();
    let store = MemoryStore::new();
    let mut cap = io.capability.get();
    cap[0] = 2;
    io.capability.set(cap);
    assert!(read(&io, &store).is_err());
    assert!(io.effects.borrow().is_empty());
    cap[0] = 0;
    io.capability.set(cap);
    assert!(read(&io, &store).unwrap().is_none());
    cap[0] = 1;
    io.capability.set(cap);
    io.flags.set([9, 0]);
    assert!(read(&io, &store).is_err());
}

#[test]
fn changed_journal_capability_or_source_rejects_old_rollback_before_filter_writes() {
    for change in 0..3 {
        let io = Memory::new();
        let store = MemoryStore::new();
        let plan = transaction::prepare(
            capture(&io, &store).unwrap(),
            &Target {
                configuration: target(Device::Keyboard),
                programs_enabled: false,
            },
        )
        .unwrap();
        let mut pending = None;
        transaction::execute(&io, &store, &plan, &mut pending).unwrap();
        let before = io.effects.borrow().len();
        match change {
            0 => store.revision.set(store.revision.get() + 1),
            1 => {
                let mut cap = io.capability.get();
                cap[13] = 0;
                io.capability.set(cap);
            }
            _ => io.source.borrow_mut().as_mut().unwrap().fingerprint = "external-change".into(),
        }
        assert!(transaction::rollback(&io, &store, &plan, pending.as_ref().unwrap()).is_err());
        assert_eq!(io.effects.borrow().len(), before);
    }
}

#[test]
fn program_preparation_failure_never_creates_a_recovery_record_or_filter_write() {
    let io = Memory::new();
    let store = MemoryStore::new();
    io.fail_programs.set(true);
    assert!(apply(&io, &store, &target(Device::Keyboard)).is_err());
    assert!(store.record.borrow().is_none());
    assert!(io.effects.borrow().is_empty());
}
