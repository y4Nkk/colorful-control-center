use super::*;
use model::{Effect, Kind, Values};
use std::cell::{Cell, RefCell};
#[path = "x58_tests.rs"]
mod x58_tests;

fn x170(snapshot: &Snapshot) -> model::X170 {
    let Values::X170(value) = snapshot.values().unwrap() else {
        unreachable!()
    };
    value
}
fn apply(io: &impl Io, baseline: &mut Option<Snapshot>, request: &Request) -> Result<()> {
    apply_tracked(io, baseline, request, &mut false)
}

fn initial() -> Snapshot {
    Snapshot {
        kind: Kind::X170,
        identity: "private-isolated-lightbar".into(),
        bytes: [176, 1, 3, 1, 255, 0, 0, 1, 1, 2, 3, 0, 4, 5, 6, 1, 7, 8, 9],
    }
}
struct Memory {
    current: RefCell<Snapshot>,
    sends: RefCell<Vec<[u8; 64]>>,
    saved: Cell<usize>,
    fail_send_at: Cell<usize>,
    attempts: Cell<usize>,
    fail_save: Cell<bool>,
    absent: Cell<bool>,
}
impl Memory {
    fn new() -> Self {
        Self {
            current: RefCell::new(initial()),
            sends: RefCell::new(vec![]),
            saved: Cell::new(0),
            fail_send_at: Cell::new(0),
            attempts: Cell::new(0),
            fail_save: Cell::new(false),
            absent: Cell::new(false),
        }
    }
    fn request(&self) -> Request {
        let mut values = x170(&self.current.borrow());
        values.effect = Effect::Breath;
        values.color = Some(Rgb {
            r: 20,
            g: 40,
            b: 60,
        });
        Request {
            expected_revision: self.current.borrow().revision(),
            values: Values::X170(values),
        }
    }
}
impl Io for Memory {
    fn ensure_available(&self) -> Result<()> {
        Ok(())
    }
    fn snapshot(&self) -> Result<Snapshot> {
        if self.absent.get() {
            bail!("隔离能力丢失");
        }
        Ok(self.current.borrow().clone())
    }
    fn send(&self, packet: &[u8; 64]) -> Result<()> {
        self.attempts.set(self.attempts.get() + 1);
        self.sends.borrow_mut().push(*packet);
        if self.attempts.get() == self.fail_send_at.get() {
            bail!("隔离HID发送失败");
        }
        Ok(())
    }
    fn save(&self, bytes: &[u8; 19]) -> Result<()> {
        self.saved.set(self.saved.get() + 1);
        self.current.borrow_mut().bytes = *bytes;
        if self.fail_save.replace(false) {
            bail!("隔离保存部分失败");
        }
        Ok(())
    }
}
#[test]
fn exact_feature_header_and_eight_modes_match_original_library() {
    let before = initial();
    for effect in model::EFFECTS {
        let values = before.for_effect(effect);
        let next = before.target(&Values::X170(values)).unwrap();
        let packets = next.packets().unwrap();
        assert_eq!(packets.len(), 3);
        assert_eq!(&packets[0][..3], &[0xcc, 191, 0]);
        assert_eq!(&packets[1][..2], &[0xcc, effect.command()]);
        assert_eq!(packets[1][3], 1);
        assert!(packets[1][7..].iter().all(|b| *b == 0));
        assert_eq!(&packets[2][..3], &[0xcc, 191, 3]);
    }
    let packets = before.packets().unwrap();
    assert_eq!(&packets[1][2..7], &[1, 1, 255, 0, 0]);
}
#[test]
fn x170_capability_is_distinct_from_dtt_and_regular_light_bar() {
    let mut flags = [0; 24];
    flags[0] = 1;
    flags[7] = 1 << 4;
    assert!(model::kinds(&flags).unwrap().is_empty());
    flags[11] = 1 << 4;
    assert!(model::kinds(&flags).unwrap().is_empty());
    flags[11] |= 1 << 3;
    assert_eq!(model::kinds(&flags).unwrap(), vec![Kind::X170]);
    flags[0] = 0;
    assert!(model::kinds(&flags).unwrap().is_empty());
    flags[0] = 2;
    assert!(model::kinds(&flags).is_err());
    assert!(model::kinds(&flags[..23]).is_err());
}
#[test]
fn draft_changes_preserve_other_modes_and_off_never_uses_saved_nonzero_brightness() {
    let before = initial();
    let mut values = x170(&before);
    values.effect = Effect::Blink;
    values.color = Some(Rgb { r: 9, g: 8, b: 7 });
    values.enabled = false;
    let target = before.target(&Values::X170(values)).unwrap();
    assert_eq!(&target.bytes[4..11], &before.bytes[4..11]);
    assert_eq!(&target.bytes[15..], &before.bytes[15..]);
    assert_eq!(&target.bytes[11..15], &[1, 9, 8, 7]);
    assert_eq!(target.packets().unwrap().last().unwrap()[2], 0);
}
#[test]
fn unknown_current_can_be_read_without_defaults_and_valid_initial_can_still_restore() {
    let io = Memory::new();
    let baseline = Some(initial());
    io.current.borrow_mut().bytes[0] = 0;
    assert!(io.current.borrow().values().is_err());
    let request = Request {
        expected_revision: io.current.borrow().revision(),
        values: initial().values().unwrap(),
    };
    assert!(apply(&io, &mut None, &request).is_err());
    assert!(io.sends.borrow().is_empty());
    let revision = io.current.borrow().revision();
    restore_initial(&io, &baseline, &revision).unwrap();
    assert_eq!(io.current.borrow().bytes, initial().bytes);
}
#[test]
fn each_send_and_partial_save_failure_compensates_and_keeps_first_baseline() {
    for failure in 1..=4 {
        let io = Memory::new();
        let request = io.request();
        let mut baseline = None;
        if failure <= 3 {
            io.fail_send_at.set(failure);
        } else {
            io.fail_save.set(true);
        }
        assert!(apply(&io, &mut baseline, &request).is_err());
        assert_eq!(io.current.borrow().bytes, initial().bytes);
        assert_eq!(baseline.unwrap().bytes, initial().bytes);
        assert_eq!(io.sends.borrow().last().unwrap()[2], 3);
    }
}
#[test]
fn failed_immediate_restore_still_saves_original_and_can_retry() {
    let io = Memory::new();
    let mut baseline = None;
    apply(&io, &mut baseline, &io.request()).unwrap();
    io.fail_send_at.set(io.attempts.get() + 1);
    let revision = io.current.borrow().revision();
    assert!(restore_initial(&io, &baseline, &revision).is_err());
    assert_eq!(io.current.borrow().bytes, initial().bytes);
    let revision = io.current.borrow().revision();
    restore_initial(&io, &baseline, &revision).unwrap();
}
#[test]
fn stale_preview_device_replacement_and_lost_capability_never_write() {
    let io = Memory::new();
    let mut request = io.request();
    request.expected_revision = "old".into();
    assert!(apply(&io, &mut None, &request).is_err());
    let mut baseline = Some(initial());
    io.current.borrow_mut().identity = "different".into();
    assert!(
        profile(&io.current.borrow(), &baseline, None)
            .unwrap()
            .blocked_reason
            .is_some()
    );
    assert!(apply(&io, &mut baseline, &io.request()).is_err());
    assert!(restore_initial(&io, &baseline, &io.current.borrow().revision()).is_err());
    io.absent.set(true);
    assert!(apply(&io, &mut baseline, &request).is_err());
    assert!(io.sends.borrow().is_empty());
    assert_eq!(io.saved.get(), 0);
    assert!(baseline.is_some());
}
#[test]
fn invalid_contracts_and_unchanged_values_do_not_write() {
    let io = Memory::new();
    let mut request = Request {
        expected_revision: io.current.borrow().revision(),
        values: initial().values().unwrap(),
    };
    let mut baseline = None;
    apply(&io, &mut baseline, &request).unwrap();
    assert!(baseline.is_none());
    for value in [5, 255] {
        let Values::X170(settings) = &mut request.values else {
            unreachable!()
        };
        settings.brightness = value;
        assert!(apply(&io, &mut baseline, &request).is_err());
    }
    request.values = initial().values().unwrap();
    let Values::X170(settings) = &mut request.values else {
        unreachable!()
    };
    settings.speed = 3;
    assert!(apply(&io, &mut baseline, &request).is_err());
    assert!(
        serde_json::from_str::<Values>(
            r#"{"enabled":true,"brightness":3,"speed":1,"effect":"legacy","color":null}"#
        )
        .is_err()
    );
    assert!(io.sends.borrow().is_empty());
    assert_eq!(io.saved.get(), 0);
}
