use super::*;
use std::cell::{Cell, RefCell};
struct Fake {
    source: Source,
    calls: RefCell<Vec<String>>,
    fail: Cell<bool>,
}
impl Fake {
    fn new() -> Self {
        Self {
            source: Source {
                panels: Inventory {
                    monitors: vec![],
                    unresolved: vec![],
                },
                nvsr_uid: Some(17),
                oled: true,
            },
            calls: RefCell::new(vec![]),
            fail: Cell::new(false),
        }
    }
}
impl Io for Fake {
    fn current(&self) -> Result<Source> {
        Ok(self.source.clone())
    }
    fn ensure(&self) -> Result<()> {
        Ok(())
    }
    fn nvsr(&self, uid: u32) -> Result<()> {
        self.calls.borrow_mut().push(format!("nvsr:{uid}"));
        if self.fail.get() {
            bail!("CUI failure");
        }
        Ok(())
    }
    fn oled(&self) -> Result<()> {
        self.calls.borrow_mut().push("oled:94/1".into());
        Ok(())
    }
}
#[test]
fn event_requests_both_branches_once_and_startup_only_nvsr() {
    let io = Fake::new();
    let identity = io.source.identity().unwrap();
    run(&io, &identity, false, || Ok(())).unwrap();
    assert_eq!(*io.calls.borrow(), ["nvsr:17", "oled:94/1"]);
    io.calls.borrow_mut().clear();
    run(&io, &identity, true, || Ok(())).unwrap();
    assert_eq!(*io.calls.borrow(), ["nvsr:17"]);
}
#[test]
fn source_mismatch_and_cancel_have_no_output() {
    let io = Fake::new();
    assert!(run(&io, "old", false, || Ok(())).is_err());
    assert!(
        run(&io, &io.source.identity().unwrap(), false, || bail!(
            "cancelled"
        ))
        .is_err()
    );
    assert!(io.calls.borrow().is_empty());
}
#[test]
fn independent_oled_request_survives_nvsr_failure_with_visible_error() {
    let io = Fake::new();
    io.fail.set(true);
    let error = run(&io, &io.source.identity().unwrap(), false, || Ok(())).unwrap_err();
    assert!(error.to_string().contains("CUI failure"));
    assert!(error.to_string().contains("OLED唤醒"));
    assert_eq!(io.calls.borrow().len(), 2);
}
#[test]
fn whitelist_matches_original_unique_edid_signatures() {
    assert_eq!(NVSR_SIGNATURES.len(), 11);
    assert!(!NVSR_SIGNATURES.contains(&"4C8329A0"));
    assert!(NVSR_SIGNATURES.contains(&"0278B3AB"));
}
