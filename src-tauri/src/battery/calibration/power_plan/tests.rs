use super::*;
use std::{cell::RefCell, collections::BTreeSet};

const BEFORE: &str = "381b4222-f694-41f0-9685-ff5bb260df2e";
fn target() -> String {
    format(owned("1234567890abcdef1234567890abcdef").unwrap())
}
struct Memory {
    active: RefCell<String>,
    schemes: RefCell<BTreeSet<String>>,
    log: RefCell<Vec<String>>,
    fail: RefCell<Option<&'static str>>,
}
impl Memory {
    fn new() -> Self {
        Self {
            active: RefCell::new(BEFORE.into()),
            schemes: RefCell::new([BEFORE.into()].into()),
            log: RefCell::new(vec![]),
            fail: RefCell::new(None),
        }
    }
    fn operation(&self, name: &'static str) -> Result<()> {
        self.log.borrow_mut().push(name.into());
        if *self.fail.borrow() == Some(name) {
            bail!("fake {name} failure");
        }
        Ok(())
    }
}
impl Access for Memory {
    fn active(&self) -> Result<String> {
        Ok(self.active.borrow().clone())
    }
    fn exists(&self, id: &str) -> Result<bool> {
        Ok(self.schemes.borrow().contains(id))
    }
    fn import(&self, id: &str) -> Result<()> {
        self.schemes.borrow_mut().insert(id.into());
        self.operation("import")
    }
    fn verify(&self, id: &str) -> Result<()> {
        if !self.schemes.borrow().contains(id) {
            bail!("fake missing plan");
        }
        self.operation("verify")
    }
    fn activate(&self, id: &str) -> Result<()> {
        if !self.schemes.borrow().contains(id) {
            bail!("fake missing plan");
        }
        self.operation("activate")?;
        *self.active.borrow_mut() = id.into();
        Ok(())
    }
    fn delete(&self, id: &str) -> Result<()> {
        assert_ne!(*self.active.borrow(), id);
        self.operation("delete")?;
        self.schemes.borrow_mut().remove(id);
        Ok(())
    }
    fn cleanup(&self) -> Result<()> {
        self.operation("cleanup")
    }
}
#[test]
fn bundled_indexes_are_bound_to_original_bytes_and_include_critical_actions() {
    let bundle = bundle().unwrap();
    assert_eq!(bundle.entries.len(), 15);
    assert_eq!(
        bundle
            .entries
            .iter()
            .map(|e| usize::from(e.ac.is_some()) + usize::from(e.dc.is_some()))
            .sum::<usize>(),
        27
    );
    let critical = bundle
        .entries
        .iter()
        .find(|e| e.setting == "637ea02f-bbcb-4015-8e2c-a1c7b9c0b546")
        .unwrap();
    assert_eq!((critical.ac, critical.dc), (Some(0), Some(0)));
    let threshold = bundle
        .entries
        .iter()
        .find(|e| e.setting == "9a66d8d7-4ff7-4ef9-b5a2-5a326ca2a469")
        .unwrap();
    assert_eq!((threshold.ac, threshold.dc), (Some(1), Some(1)));
}
#[test]
fn private_plan_is_verified_before_activation_and_original_restored_before_delete() {
    let io = Memory::new();
    let target = target();
    prepare(&io, BEFORE, &target).unwrap();
    enter(&io, BEFORE, &target).unwrap();
    assert_eq!(*io.active.borrow(), target);
    assert_eq!(*io.log.borrow(), ["import", "verify", "activate", "verify"]);
    io.log.borrow_mut().clear();
    restore(&io, BEFORE, &target).unwrap();
    assert_eq!(*io.active.borrow(), BEFORE);
    assert_eq!(*io.log.borrow(), ["activate", "delete", "cleanup"]);
    assert!(!io.exists(&target).unwrap());
}
#[test]
fn changed_active_or_existing_target_refuses_before_import() {
    for occupied in [true, false] {
        let io = Memory::new();
        let target = target();
        if occupied {
            io.schemes.borrow_mut().insert(target.clone());
        } else {
            *io.active.borrow_mut() = target.clone();
        }
        assert!(prepare(&io, BEFORE, &target).is_err());
        assert!(io.log.borrow().is_empty());
    }
}
#[test]
fn import_and_readback_failures_do_not_activate_and_can_be_compensated() {
    for failed in ["import", "verify"] {
        let io = Memory::new();
        let target = target();
        prepare(&io, BEFORE, &target).unwrap();
        *io.fail.borrow_mut() = Some(failed);
        assert!(enter(&io, BEFORE, &target).is_err());
        assert_eq!(*io.active.borrow(), BEFORE);
        assert!(!io.log.borrow().iter().any(|e| e == "activate"));
        *io.fail.borrow_mut() = None;
        restore(&io, BEFORE, &target).unwrap();
        assert!(!io.exists(&target).unwrap());
    }
}
#[test]
fn failed_original_activation_preserves_private_plan_and_can_retry() {
    let io = Memory::new();
    let target = target();
    prepare(&io, BEFORE, &target).unwrap();
    enter(&io, BEFORE, &target).unwrap();
    *io.fail.borrow_mut() = Some("activate");
    assert!(restore(&io, BEFORE, &target).is_err());
    assert!(io.exists(&target).unwrap());
    assert_eq!(*io.active.borrow(), target);
    assert_eq!(io.log.borrow().last().unwrap(), "cleanup");
    *io.fail.borrow_mut() = None;
    restore(&io, BEFORE, &target).unwrap();
}
#[test]
fn delete_and_resource_cleanup_failures_keep_a_retryable_operation() {
    for failed in ["delete", "cleanup"] {
        let io = Memory::new();
        let target = target();
        prepare(&io, BEFORE, &target).unwrap();
        enter(&io, BEFORE, &target).unwrap();
        *io.fail.borrow_mut() = Some(failed);
        assert!(restore(&io, BEFORE, &target).is_err());
        assert_eq!(*io.active.borrow(), BEFORE);
        *io.fail.borrow_mut() = None;
        restore(&io, BEFORE, &target).unwrap();
        assert!(!io.exists(&target).unwrap());
    }
}
#[test]
fn missing_temporary_plan_is_idempotent_and_invalid_identifiers_never_mutate() {
    let io = Memory::new();
    let target = target();
    restore(&io, BEFORE, &target).unwrap();
    io.log.borrow_mut().clear();
    for value in [
        "",
        "not-guid",
        "00000000-0000-0000-0000-000000000000",
        "381B4222-f694-41f0-9685-ff5bb260df2e",
    ] {
        assert!(prepare(&io, value, &target).is_err());
    }
    assert!(prepare(&io, BEFORE, BEFORE).is_err());
    assert!(io.log.borrow().is_empty());
}

#[test]
fn resource_cleanup_only_removes_an_identical_private_copy() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("calibration.pow");
    let io = native::Native {
        resource_path: path.clone(),
    };
    io.cleanup().unwrap();
    std::fs::write(&path, b"unrelated private fixture").unwrap();
    assert!(io.cleanup().is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"unrelated private fixture");
    std::fs::write(&path, RESOURCE).unwrap();
    io.cleanup().unwrap();
    assert!(!path.exists());
}
