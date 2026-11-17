use super::*;
#[path = "store_tests.rs"]
mod store_tests;
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    rc::Rc,
};

fn capability() -> Capability {
    Capability {
        factory: Limits {
            floor: 95,
            ceiling: 100,
        },
        factory_defaulted: false,
    }
}
fn policy(enabled: bool) -> Policy {
    Policy {
        enabled,
        every_day: true,
        weekdays: 0,
        slots: [Some(Slot { start: 8, end: 18 }), None],
        automatic: true,
        limits: capability().factory,
    }
}
fn charge() -> ChargeState {
    ChargeState {
        current: super::super::model::ChargeValues {
            enabled: true,
            start: 60,
            stop: 80,
        },
        start_values: vec![40, 50, 60, 70, 80, 95],
        stop_values: vec![60, 70, 80, 90, 100],
    }
}
#[derive(Debug, PartialEq)]
enum Event {
    Save(bool),
    Command(u32),
    Charge,
}
struct Fake {
    charge: RefCell<ChargeState>,
    commands: Cell<usize>,
    fail: RefCell<Vec<usize>>,
    charge_fail: Cell<bool>,
    present: Cell<bool>,
    original_missing: Cell<bool>,
    supported: Cell<bool>,
    events: Rc<RefCell<Vec<Event>>>,
}
impl Fake {
    fn new(events: Rc<RefCell<Vec<Event>>>) -> Self {
        Self {
            charge: RefCell::new(charge()),
            commands: Cell::new(0),
            fail: RefCell::new(vec![]),
            charge_fail: Cell::new(false),
            present: Cell::new(true),
            original_missing: Cell::new(false),
            supported: Cell::new(true),
            events,
        }
    }
}
impl Io for Fake {
    fn capability(&self) -> Result<Option<Capability>> {
        Ok(self.supported.get().then(capability))
    }
    fn charge(&self) -> Result<ChargeState> {
        Ok(self.charge.borrow().clone())
    }
    fn present(&self) -> Result<bool> {
        Ok(self.present.get())
    }
    fn original(&self, _: &Capability) -> Result<Option<Policy>> {
        Ok((!self.original_missing.get()).then(|| policy(false)))
    }
    fn clock(&self) -> Clock {
        Clock {
            weekday: 2,
            hour: 13,
            minute: 45,
            second: 12,
        }
    }
    fn send(&self, command: u32) -> Result<()> {
        self.events.borrow_mut().push(Event::Command(command));
        let step = self.commands.get() + 1;
        self.commands.set(step);
        if self.fail.borrow().contains(&step) {
            bail!("isolated firmware failure {step}");
        }
        if command >> 24 == 5 {
            self.charge.borrow_mut().current.enabled = command & 1 != 0;
        }
        if command >> 24 == 6 {
            let mut charge = self.charge.borrow_mut();
            charge.current.start = command as u8;
            charge.current.stop = (command >> 8) as u8;
        }
        Ok(())
    }
    fn restore_charge(&self, target: &ChargeState) -> Result<()> {
        self.events.borrow_mut().push(Event::Charge);
        if self.charge_fail.get() {
            bail!("isolated charge recovery failure");
        }
        self.charge.borrow().same_capabilities(target)?;
        *self.charge.borrow_mut() = target.clone();
        Ok(())
    }
}
#[derive(Default)]
struct Memory {
    record: RefCell<Option<Record>>,
    saves: Cell<usize>,
    fail: RefCell<Vec<usize>>,
    events: Rc<RefCell<Vec<Event>>>,
}
impl Store for Memory {
    fn load(&self) -> Result<Option<Record>> {
        Ok(self.record.borrow().clone())
    }
    fn save(&self, expected: &Option<Record>, record: &Record) -> Result<()> {
        if self.load()? != *expected {
            bail!("隔离节能记录已变化");
        }
        record.validate()?;
        self.events
            .borrow_mut()
            .push(Event::Save(record.pending.is_some()));
        let step = self.saves.get() + 1;
        self.saves.set(step);
        if self.fail.borrow().contains(&step) {
            bail!("isolated disk failure {step}");
        }
        *self.record.borrow_mut() = Some(record.clone());
        Ok(())
    }
}
fn pair() -> (Fake, Memory) {
    let store = Memory::default();
    (Fake::new(store.events.clone()), store)
}
fn request(io: &Fake, store: &impl Store, enabled: bool) -> Request {
    Request {
        expected_revision: read(io, store).unwrap().unwrap().revision,
        policy: policy(enabled),
    }
}
#[test]
fn command_words_match_original_clock_days_hour_slots_and_thresholds() {
    let io = Fake::new(Rc::default());
    let commands = codec::commands(&policy(true), &capability(), io.clock()).unwrap();
    assert_eq!(
        commands,
        vec![
            0x05000000, 0x0104DB4C, 0x02005FFF, 0x03240200, 0x04000000, 0x0600645F, 0x05000001
        ]
    );
    assert_eq!(
        codec::commands(&policy(false), &capability(), io.clock()).unwrap(),
        vec![0x05000000, 0x03000000, 0x04000000]
    );
}
#[test]
fn invalid_days_overlap_clock_and_limits_reject_before_commands() {
    let (io, store) = pair();
    for variant in 0..5 {
        let mut r = request(&io, &store, true);
        match variant {
            0 => {
                r.policy.every_day = false;
                r.policy.weekdays = 0;
            }
            1 => r.policy.slots[1] = Some(Slot { start: 17, end: 20 }),
            2 => r.policy.slots[0] = Some(Slot { start: 23, end: 0 }),
            3 => {
                r.policy.automatic = false;
                r.policy.limits.floor = 95;
            }
            _ => r.policy.limits.ceiling = 90,
        }
        assert!(apply(&io, &store, &r).is_err());
    }
    assert!(
        codec::commands(
            &policy(true),
            &capability(),
            Clock {
                weekday: 7,
                hour: 0,
                minute: 0,
                second: 0
            }
        )
        .is_err()
    );
    assert!(store.events.borrow().is_empty());
}
#[test]
fn intent_precedes_writes_and_stop_preserves_pre_schedule_charge_state() {
    let (io, store) = pair();
    apply(&io, &store, &request(&io, &store, true)).unwrap();
    assert_eq!(store.events.borrow().first(), Some(&Event::Save(true)));
    assert_eq!(io.charge.borrow().current.start, 95);
    assert!(read(&io, &store).unwrap().unwrap().can_restore);
    apply(&io, &store, &request(&io, &store, false)).unwrap();
    assert_eq!(*io.charge.borrow(), charge());
    io.charge.borrow_mut().current.start = 50;
    apply(&io, &store, &request(&io, &store, true)).unwrap();
    apply(&io, &store, &request(&io, &store, false)).unwrap();
    assert_eq!(io.charge.borrow().current.start, 50);
    restore(&io, &store, &read(&io, &store).unwrap().unwrap().revision).unwrap();
    assert_eq!(*io.charge.borrow(), charge());
}
#[test]
fn every_firmware_failure_stops_and_recovers_policy_and_charge() {
    for step in 1..=7 {
        let (io, store) = pair();
        *io.fail.borrow_mut() = vec![step];
        assert!(apply(&io, &store, &request(&io, &store, true)).is_err());
        assert_eq!(*io.charge.borrow(), charge());
        let saved = store.load().unwrap().unwrap();
        assert_eq!(saved.policy, Some(policy(false)));
        assert!(saved.pending.is_none());
        assert!(
            saved
                .last_error
                .unwrap()
                .contains("isolated firmware failure")
        );
    }
}
#[test]
fn failed_preparation_never_sends_and_failed_commit_recovers() {
    for failure in [1, 2] {
        let (io, store) = pair();
        *store.fail.borrow_mut() = vec![failure];
        assert!(apply(&io, &store, &request(&io, &store, true)).is_err());
        assert_eq!(*io.charge.borrow(), charge());
        if failure == 1 {
            assert_eq!(io.commands.get(), 0);
            assert!(store.load().unwrap().is_none());
        } else {
            assert!(store.load().unwrap().unwrap().pending.is_none());
        }
    }
}
#[test]
fn unresolved_record_blocks_application_and_restore_is_retryable() {
    let (io, store) = pair();
    apply(&io, &store, &request(&io, &store, true)).unwrap();
    io.charge_fail.set(true);
    assert!(restore(&io, &store, &read(&io, &store).unwrap().unwrap().revision).is_err());
    assert!(read(&io, &store).unwrap().unwrap().recovery_required);
    let count = io.commands.get();
    assert!(apply(&io, &store, &request(&io, &store, true)).is_err());
    assert_eq!(io.commands.get(), count);
    io.charge_fail.set(false);
    restore(&io, &store, &read(&io, &store).unwrap().unwrap().revision).unwrap();
    assert!(!read(&io, &store).unwrap().unwrap().recovery_required);
    assert_eq!(*io.charge.borrow(), charge());
}
#[test]
fn stale_preview_missing_battery_and_charge_changes_never_write() {
    let (io, store) = pair();
    let r = request(&io, &store, true);
    io.charge.borrow_mut().current.start = 50;
    assert!(apply(&io, &store, &r).is_err());
    let r = request(&io, &store, true);
    io.present.set(false);
    assert!(apply(&io, &store, &r).is_err());
    assert!(store.events.borrow().is_empty());
}
fn ini() -> String {
    "[MainSetting]\r\nCount=1\r\nSelectProfile=0\r\nEnergySaveStart=True\r\n[profile_0]\r\nEveryday=False\r\nAUTO=True\r\nSUN=False\r\nMON=True\r\nTUE=True\r\nWED=False\r\nTHU=False\r\nFRI=False\r\nSAT=False\r\nSchedule1_Enable=True\r\nSchedule1_Start=8\r\nSchedule1_End=17\r\nSchedule2_Enable=False\r\n".into()
}
#[test]
fn isolated_original_ini_read_decodes_inclusive_end_without_modifying_file() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("EnergySave.ini");
    let bytes: Vec<u8> = [0xFEFFu16]
        .into_iter()
        .chain(ini().encode_utf16())
        .flat_map(u16::to_le_bytes)
        .collect();
    std::fs::write(&path, &bytes).unwrap();
    let decoded = original::read(&path, &capability()).unwrap().unwrap();
    assert_eq!(decoded.weekdays, 6);
    assert_eq!(decoded.slots, [Some(Slot { start: 8, end: 18 }), None]);
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    assert!(
        original::read(&temp.path().join("missing.ini"), &capability())
            .unwrap()
            .is_none()
    );
}
#[test]
fn original_parser_rejects_missing_selected_profile_and_invalid_inputs() {
    let mut fields = BTreeMap::new();
    let mut section = String::new();
    for line in ini().lines() {
        if line.starts_with('[') {
            section = line.trim_matches(['[', ']']).into();
        } else if let Some((key, value)) = line.split_once('=') {
            fields.insert((section.clone(), key.to_string()), value.to_string());
        }
    }
    fields.insert(("MainSetting".into(), "SelectProfile".into()), "1".into());
    assert!(
        original::parse(
            |s, k| fields
                .get(&(s.into(), k.into()))
                .cloned()
                .context("missing"),
            &capability()
        )
        .is_err()
    );
}
#[test]
fn record_is_atomic_strict_and_pending_survives_reload() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("energy.json");
    let store = FileStore(path.clone());
    let io = Fake::new(Rc::default());
    apply(&io, &store, &request(&io, &store, true)).unwrap();
    assert!(charge_lock(&path).unwrap().is_some());
    let mut record = store.load().unwrap().unwrap();
    record.pending = Some(record.baseline.clone());
    store.save(&store.load().unwrap(), &record).unwrap();
    assert!(read(&io, &store).unwrap().unwrap().recovery_required);
    let original = std::fs::read(&path).unwrap();
    let mut invalid: serde_json::Value = serde_json::from_slice(&original).unwrap();
    invalid["version"] = 2.into();
    std::fs::write(&path, serde_json::to_vec(&invalid).unwrap()).unwrap();
    assert!(store.load().is_err());
    assert!(
        serde_json::from_str::<Request>(r#"{"expectedRevision":"old","enabled":true}"#).is_err()
    );
}
#[test]
fn versioned_capability_and_original_default_thresholds_are_explicit() {
    let mut caps = [0; 15];
    caps[..2].copy_from_slice(&[1, 0]);
    caps[14] = 1;
    let cap = codec::capability(&caps, &[0; 256]).unwrap().unwrap();
    assert!(cap.factory_defaulted);
    assert_eq!(
        cap.factory,
        Limits {
            floor: 95,
            ceiling: 100
        }
    );
    caps[14] = 0;
    assert!(codec::capability(&caps, &[0; 256]).unwrap().is_none());
    caps[0] = 2;
    assert!(codec::capability(&caps, &[0; 256]).is_err());
}

#[test]
fn first_use_without_saved_oem_plan_keeps_absence_and_declared_release_semantics() {
    let (io, store) = pair();
    io.original_missing.set(true);
    let profile = read(&io, &store).unwrap().unwrap();
    assert!(profile.saved.is_none());
    assert_eq!(profile.source, "unconfigured");
    assert!(!profile.has_initial_plan);
    apply(&io, &store, &request(&io, &store, true)).unwrap();
    assert!(store.load().unwrap().unwrap().baseline.policy.is_none());
    let profile = read(&io, &store).unwrap().unwrap();
    assert!(!profile.has_initial_plan);
    restore(&io, &store, &profile.revision).unwrap();
    assert!(read(&io, &store).unwrap().unwrap().saved.is_none());
    assert_eq!(*io.charge.borrow(), charge());
    let events = io.events.borrow();
    assert!(events.ends_with(&[
        Event::Command(0x05000000),
        Event::Command(0x03000000),
        Event::Command(0x04000000),
        Event::Charge,
        Event::Save(false)
    ]));
}

#[test]
fn unsupported_capability_has_no_profile_and_no_write_path() {
    let (io, store) = pair();
    io.supported.set(false);
    assert!(read(&io, &store).unwrap().is_none());
    assert!(
        apply(
            &io,
            &store,
            &Request {
                expected_revision: "not-supported".into(),
                policy: policy(true)
            }
        )
        .is_err()
    );
    assert!(io.events.borrow().is_empty());
}

#[test]
fn empty_original_collection_is_absent_without_guessing_a_selected_profile() {
    assert!(
        original::parse(
            |section, key| {
                assert_eq!((section, key), ("MainSetting", "Count"));
                Ok("0".into())
            },
            &capability()
        )
        .unwrap()
        .is_none()
    );
}

#[test]
fn failed_replay_releases_all_time_slots_before_charge_recovery() {
    let (io, _) = pair();
    *io.fail.borrow_mut() = vec![2];
    let snapshot = Snapshot {
        policy: Some(policy(true)),
        charge: charge(),
        capability: capability(),
    };
    assert!(recover(&io, &snapshot).is_err());
    assert!(io.events.borrow().ends_with(&[
        Event::Command(0x05000000),
        Event::Command(0x03000000),
        Event::Command(0x04000000),
        Event::Charge,
    ]));
    assert_eq!(*io.charge.borrow(), charge());
}

#[test]
fn changed_charge_candidates_refuse_a_new_write_that_cannot_restore_baseline() {
    let (io, store) = pair();
    apply(&io, &store, &request(&io, &store, true)).unwrap();
    io.charge.borrow_mut().start_values.push(90);
    let count = io.commands.get();
    assert!(apply(&io, &store, &request(&io, &store, true)).is_err());
    assert_eq!(io.commands.get(), count);
}
