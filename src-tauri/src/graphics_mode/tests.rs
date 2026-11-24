use super::*;
use std::{cell::RefCell, fs};
use tempfile::TempDir;
pub(super) struct Fake {
    pub(super) observed: RefCell<Option<Observed>>,
    identity: RefCell<Identity>,
    pub(super) sent: RefCell<Vec<u8>>,
    pub(super) fail_send: RefCell<Vec<usize>>,
    panic_send: RefCell<bool>,
    pending: RefCell<Option<u8>>,
    pub(super) reboots: RefCell<usize>,
    fail_reboot: RefCell<bool>,
}
impl Fake {
    fn new() -> Self {
        Self {
            observed: RefCell::new(Some(Observed {
                mode: 3,
                available_mask: 15,
            })),
            identity: RefCell::new(Identity {
                owner: "a".repeat(64),
                boot: "1".repeat(32),
            }),
            sent: RefCell::new(Vec::new()),
            fail_send: RefCell::new(Vec::new()),
            panic_send: RefCell::new(false),
            pending: RefCell::new(None),
            reboots: RefCell::new(0),
            fail_reboot: RefCell::new(false),
        }
    }
    pub(super) fn request(&self, store: &impl Store, target: u8) -> Request {
        Request {
            expected_revision: profile(self, store).unwrap().revision,
            target,
        }
    }
    pub(super) fn boot_to(&self, boot: char, mode: u8) {
        self.identity.borrow_mut().boot = boot.to_string().repeat(32);
        self.observed.borrow_mut().as_mut().unwrap().mode = mode;
    }
}
impl Io for Fake {
    fn observed(&self) -> Result<Option<Observed>> {
        Ok(self.observed.borrow().clone())
    }
    fn identity(&self) -> Result<Identity> {
        Ok(self.identity.borrow().clone())
    }
    fn send(&self, target: u8) -> Result<()> {
        self.sent.borrow_mut().push(target);
        *self.pending.borrow_mut() = Some(target);
        assert!(!self.panic_send.replace(false), "模拟发送后进程中断");
        if self.fail_send.borrow().contains(&self.sent.borrow().len()) {
            bail!("模拟发送失败（可能已写入）");
        }
        Ok(())
    }
    fn reboot(&self) -> Result<()> {
        *self.reboots.borrow_mut() += 1;
        if *self.fail_reboot.borrow() {
            bail!("模拟Windows拒绝重启");
        }
        Ok(())
    }
}
struct FaultStore {
    inner: FileStore,
    calls: RefCell<usize>,
    fail: Vec<usize>,
}
impl Store for FaultStore {
    fn load(&self) -> Result<Loaded> {
        self.inner.load()
    }
    fn replace(&self, before: &Loaded, record: &Record) -> Result<Loaded> {
        *self.calls.borrow_mut() += 1;
        if self.fail.contains(&*self.calls.borrow()) {
            bail!("模拟记录保存失败");
        }
        self.inner.replace(before, record)
    }
}
pub(super) fn setup() -> (TempDir, FileStore, Fake) {
    let root = tempfile::tempdir().unwrap();
    let store = FileStore(root.path().join("mode.json"));
    (root, store, Fake::new())
}
pub(super) fn verification(io: &impl Io, store: &impl Store) -> Verification {
    profile(io, store)
        .unwrap()
        .last_request
        .unwrap()
        .verification
}
#[test]
fn accepted_selection_keeps_current_getter_and_requires_real_new_boot() {
    let (_root, store, io) = setup();
    apply(&io, &store, &io.request(&store, 2)).unwrap();
    let state = profile(&io, &store).unwrap();
    assert_eq!(state.current.unwrap().mode, 3);
    assert_eq!(state.last_request.unwrap().target, 2);
    assert_eq!(verification(&io, &store), Verification::PendingReboot);
    io.observed.borrow_mut().as_mut().unwrap().mode = 2;
    assert_eq!(
        verification(&io, &store),
        Verification::PendingReboot,
        "同次启动报告碰巧匹配仍不能当作重启后确认"
    );
    io.boot_to('2', 2);
    assert_eq!(verification(&io, &store), Verification::Confirmed);
}
#[test]
fn application_reopen_retains_pending_choice() {
    let (_root, store, io) = setup();
    apply(&io, &store, &io.request(&store, 2)).unwrap();
    let reopened = FileStore(store.0.clone());
    assert_eq!(verification(&io, &reopened), Verification::PendingReboot);
    apply(&io, &reopened, &io.request(&reopened, 4)).unwrap();
    assert_eq!(*io.sent.borrow(), vec![2, 4]);
    assert_eq!(
        store.load().unwrap().record.unwrap().previous_mode,
        2,
        "再次选择以已接受的选择为回退目标"
    );
}
#[test]
fn new_boot_reports_unapplied_target_instead_of_success() {
    let (_root, store, io) = setup();
    apply(&io, &store, &io.request(&store, 2)).unwrap();
    io.boot_to('2', 3);
    assert_eq!(verification(&io, &store), Verification::NotApplied);
    assert!(reboot(&io, &store, &profile(&io, &store).unwrap().revision).is_err());
    apply(&io, &store, &io.request(&store, 2)).unwrap();
    assert_eq!(verification(&io, &store), Verification::PendingReboot);
}
#[test]
fn new_change_after_boot_records_reported_mode_as_previous_choice() {
    let (_root, store, io) = setup();
    apply(&io, &store, &io.request(&store, 2)).unwrap();
    io.boot_to('2', 2);
    apply(&io, &store, &io.request(&store, 4)).unwrap();
    assert_eq!(store.load().unwrap().record.unwrap().previous_mode, 2);
    assert_eq!(*io.sent.borrow(), vec![2, 4]);
}
#[test]
fn stale_boot_mode_or_record_revision_refuses_before_hardware() {
    for change in 0..3 {
        let (_root, store, io) = setup();
        let request = io.request(&store, 2);
        match change {
            0 => io.boot_to('2', 3),
            1 => io.observed.borrow_mut().as_mut().unwrap().mode = 4,
            _ => {
                apply(&io, &store, &io.request(&store, 4)).unwrap();
                io.sent.borrow_mut().clear();
            }
        }
        assert!(apply(&io, &store, &request).is_err());
        assert!(io.sent.borrow().is_empty());
    }
}
#[test]
fn invalid_target_and_unsupported_interface_do_not_create_record() {
    let (_root, store, io) = setup();
    for target in [0, 5, 255] {
        assert!(apply(&io, &store, &io.request(&store, target)).is_err());
    }
    assert!(!store.0.exists());
    assert!(io.sent.borrow().is_empty());
    *io.observed.borrow_mut() = None;
    assert!(profile(&io, &store).unwrap().current.is_none());
    assert!(
        apply(
            &io,
            &store,
            &Request {
                expected_revision: "unsupported".into(),
                target: 2
            }
        )
        .is_err()
    );
}
#[test]
fn prepare_failure_prevents_hardware_and_ack_save_failure_compensates() {
    for failure in [1, 2] {
        let root = tempfile::tempdir().unwrap();
        let io = Fake::new();
        let store = FaultStore {
            inner: FileStore(root.path().join("mode.json")),
            calls: RefCell::new(0),
            fail: vec![failure],
        };
        assert!(apply(&io, &store, &io.request(&store, 2)).is_err());
        if failure == 1 {
            assert!(io.sent.borrow().is_empty());
            assert!(!store.inner.0.exists());
        } else {
            assert_eq!(*io.sent.borrow(), vec![2, 3]);
            assert_eq!(verification(&io, &store), Verification::PendingReboot);
            assert_eq!(store.load().unwrap().record.unwrap().target, 3);
        }
    }
}
#[test]
fn partial_send_restores_last_known_pending_choice_not_old_boot_mode() {
    let (_root, store, io) = setup();
    apply(&io, &store, &io.request(&store, 2)).unwrap();
    *io.fail_send.borrow_mut() = vec![2];
    let error = apply(&io, &store, &io.request(&store, 4)).unwrap_err();
    assert!(error.to_string().contains("重启后核对"));
    assert_eq!(*io.sent.borrow(), vec![2, 4, 2]);
    let record = store.load().unwrap().record.unwrap();
    assert_eq!(record.target, 2);
}
#[test]
fn compensation_failure_keeps_uncertain_record_and_new_selection_resolves() {
    let (_root, store, io) = setup();
    *io.fail_send.borrow_mut() = vec![1, 2];
    assert!(
        apply(&io, &store, &io.request(&store, 2))
            .unwrap_err()
            .to_string()
            .contains("恢复未完成")
    );
    assert_eq!(verification(&io, &store), Verification::Uncertain);
    assert!(reboot(&io, &store, &profile(&io, &store).unwrap().revision).is_err());
    apply(&io, &store, &io.request(&store, 3)).unwrap();
    assert_eq!(store.load().unwrap().record.unwrap().target, 3);
    assert_eq!(verification(&io, &store), Verification::PendingReboot);
    assert_eq!(*io.sent.borrow(), vec![2, 3, 3]);
}
#[test]
fn interrupted_after_send_retains_prepared_intent_and_new_boot_can_confirm() {
    let (_root, store, io) = setup();
    *io.panic_send.borrow_mut() = true;
    let request = io.request(&store, 2);
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| apply(
            &io, &store, &request
        )))
        .is_err()
    );
    assert_eq!(
        verification(&io, &FileStore(store.0.clone())),
        Verification::Uncertain
    );
    io.boot_to('2', 2);
    assert_eq!(verification(&io, &store), Verification::Confirmed);
}
#[test]
fn reboot_is_separately_bound_and_failure_does_not_lose_choice() {
    let (_root, store, io) = setup();
    assert!(reboot(&io, &store, &profile(&io, &store).unwrap().revision).is_err());
    apply(&io, &store, &io.request(&store, 2)).unwrap();
    let revision = profile(&io, &store).unwrap().revision;
    assert!(reboot(&io, &store, "stale").is_err());
    assert_eq!(*io.reboots.borrow(), 0);
    *io.fail_reboot.borrow_mut() = true;
    assert!(reboot(&io, &store, &revision).is_err());
    assert_eq!(store.load().unwrap().record.unwrap().target, 2);
    *io.fail_reboot.borrow_mut() = false;
    reboot(&io, &store, &revision).unwrap();
    assert_eq!(*io.reboots.borrow(), 2);
}
#[test]
fn another_windows_install_and_changed_capabilities_do_not_overwrite_record() {
    for change in 0..2 {
        let (_root, store, io) = setup();
        apply(&io, &store, &io.request(&store, 2)).unwrap();
        let bytes = fs::read(&store.0).unwrap();
        if change == 0 {
            io.identity.borrow_mut().owner = "b".repeat(64);
        } else {
            io.observed.borrow_mut().as_mut().unwrap().available_mask = 7;
        }
        assert!(profile(&io, &store).is_err());
        assert_eq!(fs::read(&store.0).unwrap(), bytes);
    }
}
#[test]
fn new_boot_uses_its_current_mode_capabilities_for_new_selection() {
    let (_root, store, io) = setup();
    apply(&io, &store, &io.request(&store, 2)).unwrap();
    io.boot_to('2', 2);
    io.observed.borrow_mut().as_mut().unwrap().available_mask = 6;
    assert_eq!(verification(&io, &store), Verification::Confirmed);
    apply(&io, &store, &io.request(&store, 3)).unwrap();
    assert_eq!(store.load().unwrap().record.unwrap().available_mask, 6);
    assert_eq!(*io.pending.borrow(), Some(3));
}
#[test]
fn record_parser_rejects_unknown_version_fields_and_oversize_without_replacement() {
    let (_root, store, io) = setup();
    apply(&io, &store, &io.request(&store, 2)).unwrap();
    let valid = fs::read(&store.0).unwrap();
    let mut future: serde_json::Value = serde_json::from_slice(&valid).unwrap();
    future["version"] = (RECORD_VERSION + 1).into();
    for bad in [future, serde_json::json!({"unexpected":true})] {
        fs::write(&store.0, serde_json::to_vec(&bad).unwrap()).unwrap();
        assert!(store.load().is_err());
    }
    let mut value: serde_json::Value = serde_json::from_slice(&valid).unwrap();
    value["obsolete"] = true.into();
    fs::write(&store.0, serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(store.load().is_err());
    fs::write(&store.0, vec![b' '; 4097]).unwrap();
    assert!(store.load().is_err());
    assert_eq!(fs::metadata(&store.0).unwrap().len(), 4097);
}
#[test]
fn record_from_older_version_is_discarded_and_replaced_by_next_write() {
    let (_root, store, io) = setup();
    apply(&io, &store, &io.request(&store, 2)).unwrap();
    let mut old: serde_json::Value =
        serde_json::from_slice(&fs::read(&store.0).unwrap()).unwrap();
    old["version"] = (RECORD_VERSION - 1).into();
    old["initialMode"] = 3.into();
    fs::write(&store.0, serde_json::to_vec(&old).unwrap()).unwrap();
    assert!(store.load().unwrap().record.is_none());
    assert!(profile(&io, &store).unwrap().last_request.is_none());
    apply(&io, &store, &io.request(&store, 4)).unwrap();
    assert_eq!(verification(&io, &store), Verification::PendingReboot);
    let saved: serde_json::Value =
        serde_json::from_slice(&fs::read(&store.0).unwrap()).unwrap();
    assert_eq!(saved["version"].as_u64(), Some(u64::from(RECORD_VERSION)));
    assert!(saved.get("initialMode").is_none());
}
#[test]
fn file_compare_and_replace_rejects_external_revision_change() {
    let (_root, store, io) = setup();
    apply(&io, &store, &io.request(&store, 2)).unwrap();
    let stale = store.load().unwrap();
    let mut changed = stale.record.clone().unwrap();
    changed.target = 4;
    store.replace(&stale, &changed).unwrap();
    let bytes = fs::read(&store.0).unwrap();
    assert!(
        store
            .replace(&stale, stale.record.as_ref().unwrap())
            .is_err()
    );
    assert_eq!(fs::read(&store.0).unwrap(), bytes);
}
#[test]
fn already_observed_and_confirmed_targets_do_not_repeat_writes() {
    let (_root, store, io) = setup();
    apply(&io, &store, &io.request(&store, 3)).unwrap();
    assert!(!store.0.exists());
    apply(&io, &store, &io.request(&store, 2)).unwrap();
    io.boot_to('2', 2);
    apply(&io, &store, &io.request(&store, 2)).unwrap();
    assert_eq!(*io.sent.borrow(), vec![2]);
}
