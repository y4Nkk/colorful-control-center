use super::*;
use std::cell::{Cell, RefCell};
mod native_tests;

struct Memory {
    pages: RefCell<[[u8; 256]; 8]>,
    running: Cell<bool>,
    duties: Cell<[f64; 2]>,
    commands: RefCell<Vec<(u8, u32)>>,
    reads: Cell<usize>,
    fail_command: Cell<Option<u8>>,
    fail_all: Cell<bool>,
    fail_clock: Cell<bool>,
    ignore_save: Cell<bool>,
    fail_save: Cell<bool>,
    clocks: Cell<usize>,
}
impl Memory {
    fn new() -> Self {
        let mut pages = [[0; 256]; 8];
        pages[7][0] = 1;
        pages[7][10] = 128;
        pages[4][80] = 9;
        pages[4][85] = 73;
        pages[4][86] = 64;
        Self {
            pages: RefCell::new(pages),
            running: Cell::new(false),
            duties: Cell::new([30.0, 40.0]),
            commands: Default::default(),
            reads: Cell::new(0),
            fail_command: Cell::new(None),
            fail_all: Cell::new(false),
            fail_clock: Cell::new(false),
            ignore_save: Cell::new(false),
            fail_save: Cell::new(false),
            clocks: Cell::new(0),
        }
    }
    fn profile(&self, state: &mut State, software: bool) -> Profile {
        read(self, state, software, None).unwrap().unwrap().1
    }
    fn request(&self, state: &mut State, software: bool, schedule: Schedule) -> Request {
        Request {
            expected_revision: self.profile(state, software).revision,
            schedule,
        }
    }
    fn saved(&self) -> [u8; 5] {
        field(self, 4, 81).unwrap()
    }
}
impl SettingsAccess for Memory {
    fn read(&self, page: u8, offset: usize, length: usize) -> std::result::Result<Vec<u8>, String> {
        self.reads.set(self.reads.get() + 1);
        Ok(self.pages.borrow()[usize::from(page)][offset..offset + length].to_vec())
    }
    fn write(&self, page: u8, offset: usize, data: &[u8]) -> std::result::Result<(), String> {
        if !self.ignore_save.replace(false) {
            self.pages.borrow_mut()[usize::from(page)][offset..offset + data.len()]
                .copy_from_slice(data);
        }
        if self.fail_save.replace(false) {
            return Err("隔离保存部分失败".into());
        }
        Ok(())
    }
}
impl Io for Memory {
    fn running(&self) -> Result<bool> {
        Ok(self.running.get())
    }
    fn duties(&self) -> Result<[f64; 2]> {
        Ok(self.duties.get())
    }
    fn sync_clock(&self) -> Result<()> {
        self.clocks.set(self.clocks.get() + 1);
        if self.fail_clock.get() {
            bail!("隔离时钟失败");
        }
        Ok(())
    }
    fn command(&self, sub: u8, value: u32) -> Result<()> {
        self.commands.borrow_mut().push((sub, value));
        let fail_once = self.fail_command.get() == Some(sub);
        if fail_once {
            self.fail_command.set(None);
        }
        if fail_once || self.fail_all.get() {
            bail!("隔离除尘命令失败");
        }
        Ok(())
    }
}
fn schedule() -> Schedule {
    Schedule {
        weekdays: 65,
        hour: 23,
        on_resume: true,
    }
}
fn disabled() -> Schedule {
    Schedule {
        weekdays: 0,
        hour: 0,
        on_resume: false,
    }
}

#[test]
fn schedule_and_clock_follow_the_original_exact_encoding() {
    let s = schedule();
    s.validate().unwrap();
    assert_eq!(s.command(), 0x17c1);
    assert_eq!(s.saved(), [1, 193, 23, 0, 0]);
    assert_eq!(protocol::decode(&s.saved()).unwrap(), Some(s));
    assert_eq!(
        protocol::clock(6, 23, 59, 59).unwrap(),
        59 | 59 << 6 | 23 << 12 | 6 << 17
    );
    for args in [(7, 0, 0, 0), (0, 24, 0, 0), (0, 0, 60, 0), (0, 0, 0, 60)] {
        assert!(protocol::clock(args.0, args.1, args.2, args.3).is_err());
    }
    for bad in [Schedule { weekdays: 128, ..s }, Schedule { hour: 24, ..s }] {
        assert!(bad.validate().is_err());
    }
    assert!(protocol::decode(&[2, 0, 0, 0, 0]).is_err());
    assert!(
        serde_json::from_str::<Schedule>(r#"{"weekdays":1,"hour":0,"onResume":false,"legacy":1}"#)
            .is_err()
    );
}
#[test]
fn unsupported_and_unknown_capabilities_do_not_probe_or_initialize_a_schedule() {
    let io = Memory::new();
    let mut state = State::default();
    io.pages.borrow_mut()[7][10] = 0;
    assert!(read(&io, &mut state, false, None).unwrap().is_none());
    assert_eq!(io.reads.get(), 1);
    io.pages.borrow_mut()[7][0] = 0;
    assert!(read(&io, &mut state, false, None).unwrap().is_none());
    io.pages.borrow_mut()[7][0] = 2;
    assert!(read(&io, &mut state, false, None).is_err());
    assert!(io.commands.borrow().is_empty());
    assert_eq!(io.clocks.get(), 0);
}
#[test]
fn plan_apply_and_restore_preserve_all_other_fields_and_first_raw_baseline() {
    let io = Memory::new();
    let mut state = State::default();
    let original = io.saved();
    let request = io.request(&mut state, false, schedule());
    apply(&io, &mut state, false, &request).unwrap();
    assert_eq!(io.saved(), schedule().saved());
    assert_eq!(io.commands.borrow().as_slice(), &[(40, 0x17c1)]);
    assert_eq!(io.clocks.get(), 1);
    let request = io.request(&mut state, false, disabled());
    apply(&io, &mut state, false, &request).unwrap();
    assert_eq!(state.baseline, Some(original));
    let revision = io.profile(&mut state, false).revision;
    restore(&io, &mut state, false, &revision).unwrap();
    assert_eq!(io.saved(), original);
    assert_eq!(io.commands.borrow().last(), Some(&(40, 0)));
    assert_eq!(io.pages.borrow()[4][80], 9);
    assert_eq!(io.pages.borrow()[4][86], 64);
    assert!(
        io.profile(&mut state, false)
            .initial
            .unwrap()
            .schedule
            .is_none()
    );
}
#[test]
fn old_previews_and_invalid_requests_never_send_or_create_a_baseline() {
    let io = Memory::new();
    let mut state = State::default();
    let request = io.request(&mut state, false, schedule());
    io.pages.borrow_mut()[4][82] = 1;
    assert!(apply(&io, &mut state, false, &request).is_err());
    let request = io.request(
        &mut state,
        false,
        Schedule {
            hour: 24,
            ..schedule()
        },
    );
    assert!(apply(&io, &mut state, false, &request).is_err());
    assert!(state.baseline.is_none());
    assert!(io.commands.borrow().is_empty());
}
#[test]
fn start_rechecks_full_speed_software_and_every_duty_boundary() {
    let io = Memory::new();
    let mut state = State::default();
    for duties in [[50.01, 0.0], [0.0, 51.0], [f64::NAN, 0.0], [-1.0, 0.0]] {
        io.duties.set(duties);
        let revision = io.profile(&mut state, false).revision;
        assert!(start(&io, &mut state, false, &revision).is_err());
    }
    io.duties.set([50.0, 50.0]);
    io.pages.borrow_mut()[4][5] = 1;
    let revision = io.profile(&mut state, false).revision;
    assert!(start(&io, &mut state, false, &revision).is_err());
    io.pages.borrow_mut()[4][5] = 0;
    let revision = io.profile(&mut state, true).revision;
    assert!(start(&io, &mut state, true, &revision).is_err());
    assert!(io.commands.borrow().is_empty());
    let revision = io.profile(&mut state, false).revision;
    start(&io, &mut state, false, &revision).unwrap();
    assert_eq!(io.commands.borrow().as_slice(), &[(41, 1)]);
}
#[test]
fn start_is_consumed_even_on_transport_error_and_never_sends_a_fictitious_stop() {
    let io = Memory::new();
    let mut state = State::default();
    let revision = io.profile(&mut state, false).revision;
    io.fail_command.set(Some(41));
    assert!(start(&io, &mut state, false, &revision).is_err());
    assert_eq!(state.last_start, Some(StartOutcome::Uncertain));
    assert!(start(&io, &mut state, false, &revision).is_err());
    assert_eq!(io.commands.borrow().len(), 1);
    let revision = io.profile(&mut state, false).revision;
    start(&io, &mut state, false, &revision).unwrap();
    assert_eq!(
        io.profile(&mut state, false).last_start,
        Some(StartOutcome::Sent)
    );
    io.running.set(true);
    assert_eq!(
        io.profile(&mut state, false).last_start,
        Some(StartOutcome::Running)
    );
    io.running.set(false);
    assert_eq!(
        io.profile(&mut state, false).last_start,
        Some(StartOutcome::Completed)
    );
    assert!(
        io.commands
            .borrow()
            .iter()
            .all(|command| *command == (41, 1))
    );
    assert_eq!(io.pages.borrow()[4][80], 9);
}
#[test]
fn running_firmware_blocks_both_actions_and_schedule_restoration() {
    let io = Memory::new();
    let mut state = State::default();
    let request = io.request(&mut state, false, schedule());
    apply(&io, &mut state, false, &request).unwrap();
    io.commands.borrow_mut().clear();
    io.running.set(true);
    let request = io.request(&mut state, false, disabled());
    assert!(start(&io, &mut state, false, &request.expected_revision).is_err());
    assert!(apply(&io, &mut state, false, &request).is_err());
    assert!(restore(&io, &mut state, false, &request.expected_revision).is_err());
    assert!(ensure_idle(&io).is_err());
    assert!(io.commands.borrow().is_empty());
}
#[test]
fn software_cannot_enable_or_restore_an_enabled_or_unreported_plan() {
    let io = Memory::new();
    let mut state = State::default();
    let request = io.request(&mut state, true, schedule());
    assert!(apply(&io, &mut state, true, &request).is_err());
    assert!(io.commands.borrow().is_empty());
    let request = io.request(&mut state, true, disabled());
    apply(&io, &mut state, true, &request).unwrap();
    let revision = io.profile(&mut state, true).revision;
    assert!(restore(&io, &mut state, true, &revision).is_err());
    let revision = io.profile(&mut state, false).revision;
    restore(&io, &mut state, false, &revision).unwrap();
}
#[test]
fn command_or_save_partial_failure_and_ignored_write_compensate_the_exact_saved_bytes() {
    for fault in 0..3 {
        let io = Memory::new();
        let mut state = State::default();
        let original = io.saved();
        let request = io.request(&mut state, false, schedule());
        match fault {
            0 => io.fail_command.set(Some(40)),
            1 => io.fail_save.set(true),
            _ => io.ignore_save.set(true),
        }
        assert!(apply(&io, &mut state, false, &request).is_err());
        assert_eq!(io.saved(), original);
        assert_eq!(io.commands.borrow().last(), Some(&(40, 0)));
        assert_eq!(state.baseline, Some(original));
    }
}
#[test]
fn compensation_failure_retains_a_retryable_baseline_and_clock_failure_never_starts_a_plan() {
    let io = Memory::new();
    let mut state = State::default();
    let original = io.saved();
    let request = io.request(&mut state, false, schedule());
    io.fail_all.set(true);
    let error = apply(&io, &mut state, false, &request)
        .unwrap_err()
        .to_string();
    assert!(error.contains("恢复"));
    io.fail_all.set(false);
    let revision = io.profile(&mut state, false).revision;
    restore(&io, &mut state, false, &revision).unwrap();
    assert_eq!(io.saved(), original);
    io.commands.borrow_mut().clear();
    io.fail_clock.set(true);
    let request = io.request(&mut state, false, schedule());
    assert!(apply(&io, &mut state, false, &request).is_err());
    assert!(io.commands.borrow().is_empty());
}
#[test]
fn cal_interlock_fact_changes_invalidate_confirmation_and_identical_plans_do_not_write() {
    let io = Memory::new();
    let mut state = State::default();
    let revision = io.profile(&mut state, false).revision;
    let profile = read(&io, &mut state, false, Some("校准尚需恢复".into()))
        .unwrap()
        .unwrap()
        .1;
    assert_ne!(profile.revision, revision);
    assert!(review(&profile, &profile.revision, true).is_err());
    io.pages.borrow_mut()[4][81..86].copy_from_slice(&schedule().saved());
    let request = io.request(&mut state, false, schedule());
    apply(&io, &mut state, false, &request).unwrap();
    assert!(state.baseline.is_none());
    assert!(io.commands.borrow().is_empty());
}
