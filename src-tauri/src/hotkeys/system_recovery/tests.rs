use super::*;
use std::cell::{Cell, RefCell};

struct Memory {
    sources: RefCell<Vec<Source>>,
    opened: RefCell<Vec<Source>>,
    fail: Cell<bool>,
}
impl Memory {
    fn new() -> Self {
        Self {
            sources: RefCell::new(vec![Source {
                path: "isolated-system/control.exe".into(),
                fingerprint: "source-a".into(),
            }]),
            opened: RefCell::new(vec![]),
            fail: Cell::new(false),
        }
    }
    fn identity(&self) -> String {
        self.sources.borrow()[0].identity().unwrap()
    }
}
impl Io for Memory {
    fn source(&self) -> Result<Source> {
        let mut sources = self.sources.borrow_mut();
        if sources.len() > 1 {
            Ok(sources.remove(0))
        } else {
            Ok(sources[0].clone())
        }
    }
    fn open(&self, source: &Source) -> Result<()> {
        if self.fail.get() {
            bail!("isolated launch failed");
        }
        self.opened.borrow_mut().push(source.clone());
        Ok(())
    }
}

#[test]
fn confirmed_recovery_opens_only_the_bound_source_without_system_restore_operations() {
    let io = Memory::new();
    run(&io, &io.identity(), || Ok(())).unwrap();
    assert_eq!(*io.opened.borrow(), *io.sources.borrow());
}

#[test]
fn cancelled_or_revoked_at_every_boundary_never_launches() {
    for boundary in 1..=3 {
        let io = Memory::new();
        let calls = Cell::new(0);
        assert!(
            run(&io, &io.identity(), || {
                calls.set(calls.get() + 1);
                if calls.get() == boundary {
                    bail!("cancelled");
                }
                Ok(())
            })
            .is_err()
        );
        assert!(io.opened.borrow().is_empty());
    }
}

#[test]
fn changed_authorized_program_or_source_during_preparation_cannot_open() {
    let io = Memory::new();
    assert!(run(&io, "stale", || Ok(())).is_err());
    let expected = io.identity();
    let mut changed = io.sources.borrow()[0].clone();
    changed.fingerprint = "source-b".into();
    io.sources.borrow_mut().push(changed);
    assert!(
        run(&io, &expected, || Ok(()))
            .unwrap_err()
            .to_string()
            .contains("执行前")
    );
    assert!(io.opened.borrow().is_empty());
}

#[test]
fn launch_failure_keeps_the_real_error_and_native_test_path_never_opens_windows() {
    let io = Memory::new();
    io.fail.set(true);
    assert!(
        run(&io, &io.identity(), || Ok(()))
            .unwrap_err()
            .to_string()
            .contains("launch failed")
    );
    assert!(identity().is_err());
    assert!(
        execute("anything", || panic!(
            "test must not enter native operations"
        ))
        .is_err()
    );
}
