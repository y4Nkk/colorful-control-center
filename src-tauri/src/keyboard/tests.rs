use super::*;
use std::cell::RefCell;
struct Fake {
    state: RefCell<Snapshot>,
    writes: RefCell<Vec<[u8; 5]>>,
    fail_save: RefCell<usize>,
    corrupt_readback: RefCell<bool>,
}
impl Fake {
    fn new() -> Self {
        Self {
            state: RefCell::new(Snapshot {
                live: [2, 1, 2, 7, 9],
                saved: [0, 0, 0],
            }),
            writes: RefCell::new(Vec::new()),
            fail_save: RefCell::new(0),
            corrupt_readback: RefCell::new(false),
        }
    }
    fn request(&self) -> Request {
        Request {
            expected_revision: read(self).unwrap().revision(),
            values: Values {
                fn_locked: Some(true),
                win_locked: Some(true),
                fn_win_swap: Some(true),
            },
        }
    }
}
impl Io for Fake {
    fn live(&self) -> Result<[u8; 5]> {
        Ok(self.state.borrow().live)
    }
    fn saved(&self) -> Result<[u8; 3]> {
        Ok(self.state.borrow().saved)
    }
    fn send(&self, values: [u8; 5]) -> Result<()> {
        self.writes.borrow_mut().push(values);
        self.state.borrow_mut().live = values;
        if self.corrupt_readback.replace(false) {
            self.state.borrow_mut().live[0] = 2;
        }
        Ok(())
    }
    fn save(&self, values: [u8; 3]) -> Result<()> {
        self.state.borrow_mut().saved = values;
        let mut left = self.fail_save.borrow_mut();
        if *left > 0 {
            *left -= 1;
            bail!("保存失败（写入后）");
        }
        Ok(())
    }
}
#[test]
fn shared_packet_and_saved_encoding_keep_unowned_states() {
    let io = Fake::new();
    let original = read(&io).unwrap();
    let mut baseline = None;
    apply(&io, &mut baseline, &io.request()).unwrap();
    assert_eq!(read(&io).unwrap().live, [1, 2, 1, 7, 9]);
    assert_eq!(io.saved().unwrap(), [1, 2, 1]);
    assert_eq!(baseline, Some(original));
}
#[test]
fn stale_live_or_saved_preview_rejects_before_write() {
    for saved in [false, true] {
        let io = Fake::new();
        let request = io.request();
        if saved {
            io.state.borrow_mut().saved[0] = 1;
        } else {
            io.state.borrow_mut().live[4] = 8;
        }
        assert!(apply(&io, &mut None, &request).is_err());
        assert!(io.writes.borrow().is_empty());
    }
}
#[test]
fn unsupported_and_unknown_fields_never_write() {
    let io = Fake::new();
    io.state.borrow_mut().live[1] = 0;
    assert!(apply(&io, &mut None, &io.request()).is_err());
    let mut request = io.request();
    request.values.win_locked = None;
    apply(&io, &mut None, &request).unwrap();
    assert_eq!(io.live().unwrap()[1], 0);
    assert_eq!(io.saved().unwrap()[1], 0);
    io.state.borrow_mut().live[0] = 3;
    assert!(read(&io).is_err());
}
#[test]
fn partial_save_failure_restores_live_saved_and_retains_baseline() {
    let io = Fake::new();
    let original = read(&io).unwrap();
    *io.fail_save.borrow_mut() = 1;
    let mut baseline = None;
    assert!(apply(&io, &mut baseline, &io.request()).is_err());
    assert_eq!(read(&io).unwrap(), original);
    assert!(baseline.is_some());
}
#[test]
fn readback_mismatch_compensates_before_saved_write() {
    let io = Fake::new();
    let original = read(&io).unwrap();
    *io.corrupt_readback.borrow_mut() = true;
    assert!(apply(&io, &mut None, &io.request()).is_err());
    assert_eq!(read(&io).unwrap(), original);
}
#[test]
fn initial_restore_preserves_latest_unowned_fields_and_can_retry() {
    let io = Fake::new();
    let mut baseline = None;
    apply(&io, &mut baseline, &io.request()).unwrap();
    io.state.borrow_mut().live[3..].copy_from_slice(&[3, 4]);
    *io.fail_save.borrow_mut() = 1;
    assert!(restore(&io, baseline.as_ref().unwrap()).is_err());
    restore(&io, baseline.as_ref().unwrap()).unwrap();
    assert_eq!(io.live().unwrap(), [2, 1, 2, 3, 4]);
    assert_eq!(io.saved().unwrap(), [0, 0, 0]);
}
#[test]
fn changed_support_prevents_baseline_restore() {
    let io = Fake::new();
    let original = read(&io).unwrap();
    io.state.borrow_mut().live[2] = 0;
    assert!(restore(&io, &original).is_err());
    assert!(io.writes.borrow().is_empty());
}
#[test]
fn no_change_keeps_saved_defaults_and_does_not_arm_restore() {
    let io = Fake::new();
    let before = read(&io).unwrap();
    let request = Request {
        expected_revision: before.revision(),
        values: before.values(),
    };
    let mut baseline = None;
    apply(&io, &mut baseline, &request).unwrap();
    assert!(baseline.is_none());
    assert!(io.writes.borrow().is_empty());
    assert_eq!(io.saved().unwrap(), [0, 0, 0]);
}
#[test]
fn fn_only_change_preserves_other_saved_defaults_and_first_baseline() {
    let io = Fake::new();
    let original = read(&io).unwrap();
    let mut values = original.values();
    values.fn_locked = Some(true);
    let mut baseline = None;
    apply(
        &io,
        &mut baseline,
        &Request {
            expected_revision: original.revision(),
            values,
        },
    )
    .unwrap();
    assert_eq!(io.live().unwrap(), [1, 1, 2, 7, 9]);
    assert_eq!(io.saved().unwrap(), [1, 0, 0]);
    apply(&io, &mut baseline, &io.request()).unwrap();
    assert_eq!(baseline, Some(original));
}
#[test]
fn failed_compensation_reports_error_and_initial_snapshot_remains_retryable() {
    let io = Fake::new();
    let original = read(&io).unwrap();
    *io.fail_save.borrow_mut() = 2;
    let mut baseline = None;
    assert!(
        apply(&io, &mut baseline, &io.request())
            .unwrap_err()
            .to_string()
            .contains("恢复失败")
    );
    restore(&io, baseline.as_ref().unwrap()).unwrap();
    assert_eq!(read(&io).unwrap(), original);
}
