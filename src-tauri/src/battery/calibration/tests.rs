use super::*;
use crate::battery::model::ChargeValues;
use anyhow::{Result, bail};
use std::{collections::BTreeSet, time::Duration};
#[path = "worker_tests.rs"]
mod worker_tests;

struct Fake {
    facts: Facts,
    events: Vec<Event>,
    words: Vec<(i32, u32)>,
    failures: BTreeSet<usize>,
    awake: bool,
    restored: usize,
    restore_fails: bool,
    ready: bool,
    verifications: usize,
    changed_on_verification: Option<usize>,
    plan_active: bool,
    plan_entered: usize,
    plan_restored: usize,
    plan_fault: Option<&'static str>,
}
impl Default for Fake {
    fn default() -> Self {
        Self {
            facts: Facts {
                present: Some(true),
                ac_online: Some(true),
                percent: Some(60),
                battery_status: 0,
                permanent_failure: 0,
                stop_threshold: 80,
            },
            events: vec![],
            words: vec![],
            failures: BTreeSet::new(),
            awake: false,
            restored: 0,
            restore_fails: false,
            ready: true,
            verifications: 0,
            changed_on_verification: None,
            plan_active: false,
            plan_entered: 0,
            plan_restored: 0,
            plan_fault: None,
        }
    }
}
impl Access for Fake {
    fn prepare_power_plan(&mut self, _: &str) -> Result<()> {
        if self.plan_fault == Some("prepare") {
            bail!("fake occupied plan");
        }
        Ok(())
    }
    fn enter_power_plan(&mut self, _: &str) -> Result<()> {
        self.plan_entered += 1;
        self.plan_active = true;
        if self.plan_fault == Some("import") {
            bail!("fake partial import");
        }
        Ok(())
    }
    fn check_power_plan(&mut self) -> Result<()> {
        if self.plan_fault == Some("changed") {
            bail!("fake external plan change");
        }
        Ok(())
    }
    fn restore_power_plan(&mut self, _: &str) -> Result<()> {
        self.plan_restored += 1;
        if self.plan_fault == Some("restore") {
            bail!("fake plan recovery failure");
        }
        self.plan_active = false;
        Ok(())
    }
    fn verify_baseline(&mut self, baseline: &Baseline) -> Result<()> {
        self.verifications += 1;
        if self.changed_on_verification == Some(self.verifications) {
            bail!("fake baseline changed");
        }
        baseline.validate()
    }
    fn ready(&mut self) -> Result<()> {
        if !self.ready {
            bail!("fake producer unavailable");
        }
        Ok(())
    }
    fn facts(&mut self) -> Result<Facts> {
        Ok(self.facts.clone())
    }
    fn events(&mut self) -> Result<Vec<Event>> {
        Ok(std::mem::take(&mut self.events))
    }
    fn send(&mut self, cmd: i32, word: u32) -> Result<()> {
        self.words.push((cmd, word));
        if self.failures.contains(&self.words.len()) {
            bail!("fake transport failure");
        }
        Ok(())
    }
    fn keep_awake(&mut self, enabled: bool) -> Result<()> {
        self.awake = enabled;
        Ok(())
    }
    fn restore_policy(&mut self, baseline: &Baseline) -> Result<()> {
        baseline.validate()?;
        self.restored += 1;
        if self.restore_fails {
            bail!("fake policy failure");
        }
        Ok(())
    }
}
fn baseline() -> Baseline {
    Baseline {
        stop_threshold: 80,
        power_scheme: "381b4222-f694-41f0-9685-ff5bb260df2e".into(),
        charge: ChargeState {
            current: ChargeValues {
                enabled: true,
                start: 40,
                stop: 80,
            },
            start_values: vec![40, 60],
            stop_values: vec![80, 100],
        },
        energy: None,
    }
}
fn start(io: &mut Fake) -> Task {
    Task::start(io, baseline(), Duration::ZERO, &mut || Ok(())).unwrap()
}
fn event(task: &mut Task, io: &mut Fake, second: u64, status: u16, event: Event) {
    io.facts.battery_status = status;
    io.events = vec![event];
    task.tick(io, Duration::from_secs(second));
}
#[test]
fn commands_match_oem_order_and_percent_does_not_complete_a_phase() {
    let mut io = Fake::default();
    let mut task = start(&mut io);
    assert_eq!(
        io.words,
        vec![
            (121, 0x1c000001),
            (121, 0x1c000000),
            (121, 0x1d000001),
            (118, 0x07000000)
        ]
    );
    io.facts.percent = Some(100);
    io.facts.ac_online = Some(false);
    io.facts.battery_status = 0x30;
    task.tick(&mut io, Duration::from_secs(1));
    assert_eq!(task.state.phase, Phase::Discharge1);
    assert_eq!(task.state.status, Status::Running);
    assert_eq!(io.words.len(), 4);
}
#[test]
fn full_status_requires_fresh_notification_and_correct_ac_fact() {
    let mut io = Fake::default();
    let mut task = start(&mut io);
    event(&mut task, &mut io, 1, 0x10, Event::FullStatus);
    assert_eq!(task.state.phase, Phase::Discharge1);
    io.facts.ac_online = Some(false);
    event(&mut task, &mut io, 2, 0x10, Event::FullStatus);
    assert_eq!(task.state.phase, Phase::Charge1);
    assert_eq!(
        &io.words[4..],
        &[(121, 0x1c000001), (121, 0x1d000001), (118, 0x07000000)]
    );
    task.tick(&mut io, Duration::from_secs(3));
    assert_eq!(task.state.status, Status::Running);
}
#[test]
fn dedicated_discharge_notification_and_full_charge_drive_all_five_phases() {
    let mut io = Fake::default();
    let mut task = start(&mut io);
    event(&mut task, &mut io, 1, 0, Event::FullyDischarged);
    assert_eq!(task.state.phase, Phase::Charge1);
    event(&mut task, &mut io, 2, 0x20, Event::FullStatus);
    assert_eq!(task.state.phase, Phase::Discharge2);
    event(&mut task, &mut io, 3, 0, Event::FullyDischarged);
    assert_eq!(task.state.phase, Phase::Charge2);
    io.facts.percent = Some(90);
    event(&mut task, &mut io, 4, 0x20, Event::FullStatus);
    assert_eq!(task.state.phase, Phase::Adjusting);
    assert_eq!(task.state.status, Status::Running);
    let writes = io.words.len();
    task.tick(&mut io, Duration::from_secs(5));
    assert_eq!(io.words.len(), writes);
    io.facts.percent = Some(80);
    task.tick(&mut io, Duration::from_secs(6));
    assert_eq!(task.state.status, Status::Completed);
    assert_eq!(
        &io.words[io.words.len() - 3..],
        &model::commands(true, false, true)
    );
    assert_eq!(io.restored, 1);
    assert!(!io.awake);
}
#[test]
fn unplug_wins_over_completion_and_replug_preserves_elapsed_phase() {
    let mut io = Fake::default();
    let mut task = start(&mut io);
    io.facts.ac_online = Some(false);
    task.tick(&mut io, Duration::from_secs(1));
    io.events = vec![Event::FullyDischarged, Event::AcUnplugged];
    task.tick(&mut io, Duration::from_secs(2));
    assert_eq!(task.state.status, Status::Paused);
    assert_eq!(task.state.phase, Phase::Discharge1);
    assert_eq!(&io.words[4..], &model::commands(true, true, false));
    task.tick(&mut io, Duration::from_secs(3));
    assert_eq!(task.state.elapsed_ms, 1000);
    io.facts.ac_online = Some(true);
    task.tick(&mut io, Duration::from_secs(4));
    assert_eq!(task.state.status, Status::Running);
    assert_eq!(task.state.phase_elapsed_ms, 1000);
    assert_eq!(&io.words[7..], &Phase::Discharge1.commands());
}
#[test]
fn charging_offline_pauses_only_after_ac_has_been_acknowledged() {
    let mut io = Fake::default();
    let mut task = start(&mut io);
    io.facts.ac_online = Some(false);
    event(&mut task, &mut io, 1, 0, Event::FullyDischarged);
    task.tick(&mut io, Duration::from_secs(2));
    assert_eq!(task.state.status, Status::Running);
    io.facts.ac_online = Some(true);
    task.tick(&mut io, Duration::from_secs(3));
    io.facts.ac_online = Some(false);
    task.tick(&mut io, Duration::from_secs(4));
    assert_eq!(task.state.status, Status::Paused);
    assert_eq!(task.state.phase, Phase::Charge1);
}
#[test]
fn one_notification_batch_cannot_skip_multiple_stages() {
    let mut io = Fake::default();
    let mut task = start(&mut io);
    io.facts.ac_online = Some(false);
    io.events = vec![Event::FullyDischarged, Event::FullStatus, Event::FullStatus];
    io.facts.battery_status = 0x30;
    task.tick(&mut io, Duration::from_secs(1));
    assert_eq!(task.state.phase, Phase::Charge1);
}
#[test]
fn each_partial_start_failure_releases_all_controls_and_original_policy() {
    for failure in 1..=4 {
        let mut io = Fake::default();
        io.failures.insert(failure);
        let task = start(&mut io);
        assert_eq!(task.state.status, Status::Failed, "failure {failure}");
        assert_eq!(io.words.len(), failure + 3);
        assert_eq!(&io.words[failure..], &model::commands(true, false, true));
        assert_eq!(io.restored, 1);
        assert!(!io.awake);
    }
}
#[test]
fn phase_write_failure_stops_then_compensates_without_later_phase_writes() {
    let mut io = Fake::default();
    let mut task = start(&mut io);
    io.failures.insert(6);
    event(&mut task, &mut io, 1, 0, Event::FullyDischarged);
    assert_eq!(task.state.status, Status::Failed);
    assert_eq!(task.state.phase, Phase::Discharge1);
    assert_eq!(io.words.len(), 9);
    assert_eq!(&io.words[6..], &model::commands(true, false, true));
}
#[test]
fn recovery_attempts_every_release_and_policy_then_allows_retry() {
    let mut io = Fake::default();
    let mut task = start(&mut io);
    io.failures.extend([5, 6, 7]);
    io.restore_fails = true;
    task.stop(&mut io);
    assert_eq!(task.state.status, Status::RecoveryRequired);
    assert_eq!(io.words.len(), 7);
    assert_eq!(io.restored, 1);
    assert!(!io.awake);
    io.failures.clear();
    io.restore_fails = false;
    task.retry_restore(&mut io);
    assert_eq!(task.state.status, Status::Stopped);
    assert_eq!(io.restored, 2);
}
#[test]
fn unknown_power_missing_battery_failure_status_and_sleep_gap_restore() {
    for kind in 0..7 {
        let mut io = Fake::default();
        let mut task = start(&mut io);
        match kind {
            0 => io.facts.present = None,
            1 => io.facts.present = Some(false),
            2 => io.facts.ac_online = None,
            3 => io.facts.percent = None,
            4 => io.facts.permanent_failure = 0x80000000,
            5 => io.facts.stop_threshold = 0,
            _ => (),
        }
        task.tick(&mut io, Duration::from_secs(if kind == 6 { 6 } else { 1 }));
        assert_eq!(task.state.status, Status::Failed, "kind {kind}");
        assert_eq!(io.restored, 1);
    }
}
#[test]
fn unavailable_notifications_and_invalid_start_never_write() {
    for kind in 0..4 {
        let mut io = Fake::default();
        match kind {
            0 => io.ready = false,
            1 => io.facts.ac_online = Some(false),
            2 => io.facts.percent = None,
            _ => io.facts.stop_threshold = 101,
        }
        assert!(Task::start(&mut io, baseline(), Duration::ZERO, &mut || Ok(())).is_err());
        assert!(io.words.is_empty());
        assert!(!io.awake);
    }
}
#[test]
fn notification_failure_or_backlog_ends_the_task() {
    for stopped in [true, false] {
        let mut io = Fake::default();
        let mut task = start(&mut io);
        if stopped {
            io.ready = false;
        } else {
            io.events = vec![Event::FullStatus; 65];
        }
        task.tick(&mut io, Duration::from_secs(1));
        assert_eq!(task.state.status, Status::Failed);
        assert_eq!(io.restored, 1);
    }
}

fn record(number: u32, source: &str, message: &str) -> Vec<u8> {
    let mut raw = vec![0; 56];
    let mut push = |value: &str| {
        raw.extend(value.encode_utf16().chain([0]).flat_map(u16::to_le_bytes));
    };
    push(source);
    push("fixture");
    let offset = raw.len();
    raw.extend(message.encode_utf16().chain([0]).flat_map(u16::to_le_bytes));
    while raw.len() % 4 != 0 {
        raw.push(0);
    }
    raw.extend([0; 4]);
    let len = raw.len() as u32;
    raw[..4].copy_from_slice(&len.to_le_bytes());
    raw[4..8].copy_from_slice(&0x654c664cu32.to_le_bytes());
    raw[8..12].copy_from_slice(&number.to_le_bytes());
    raw[26..28].copy_from_slice(&1u16.to_le_bytes());
    raw[36..40].copy_from_slice(&(offset as u32).to_le_bytes());
    let end = raw.len();
    raw[end - 4..].copy_from_slice(&len.to_le_bytes());
    raw
}
#[test]
fn raw_log_records_filter_source_and_exact_messages_preserving_order() {
    let raw: Vec<_> = [
        ("PowerBIOSServer_Out", "Arg001^85"),
        ("other", "Arg001^90"),
        ("PowerBIOSServer_Out", "Arg001^90"),
        ("PowerBIOSServer_Out", "Arg001^91"),
        ("PowerBIOSServer_Out", "Arg001^85^1"),
    ]
    .into_iter()
    .enumerate()
    .flat_map(|(i, (s, m))| record(i as u32 + 5, s, m))
    .collect();
    let (next, events) = events::parse(&raw, 5).unwrap();
    assert_eq!(next, 10);
    assert_eq!(
        events,
        vec![
            Event::FullStatus,
            Event::AcUnplugged,
            Event::FullyDischarged
        ]
    );
}
#[test]
fn malformed_log_records_gaps_replays_and_backlog_are_rejected() {
    let valid = record(1, "PowerBIOSServer_Out", "Arg001^85");
    for kind in 0..6 {
        let mut raw = valid.clone();
        match kind {
            0 => raw[0] = 1,
            1 => raw[4] = 0,
            2 => *raw.last_mut().unwrap() = 1,
            3 => raw[8] = 2,
            4 => raw[36..40].copy_from_slice(&u32::MAX.to_le_bytes()),
            _ => raw.truncate(raw.len() - 1),
        }
        assert!(events::parse(&raw, 1).is_err(), "kind {kind}");
    }
    let raw: Vec<_> = (1..=65)
        .flat_map(|i| record(i, "PowerBIOSServer_Out", "Arg001^85"))
        .collect();
    assert!(events::parse(&raw, 1).is_err());
}

#[test]
fn changed_calibration_threshold_rejects_start_or_cancels_an_active_task() {
    let mut io = Fake::default();
    io.facts.stop_threshold = 81;
    assert!(Task::start(&mut io, baseline(), Duration::ZERO, &mut || Ok(())).is_err());
    assert!(io.words.is_empty());
    io.facts.stop_threshold = 80;
    let mut task = start(&mut io);
    io.facts.stop_threshold = 81;
    task.tick(&mut io, Duration::from_secs(1));
    assert_eq!(task.state.status, Status::Failed);
    assert_eq!(io.restored, 1);
}

#[test]
fn repeated_recovery_failures_keep_a_bounded_current_diagnostic() {
    let mut io = Fake::default();
    let mut task = start(&mut io);
    io.restore_fails = true;
    task.stop(&mut io);
    let initial = task.state.error.clone();
    for _ in 0..100 {
        task.retry_restore(&mut io);
    }
    assert_eq!(task.state.error, initial);
    io.restore_fails = false;
    task.retry_restore(&mut io);
    assert_eq!(task.state.status, Status::Stopped);
    assert!(task.state.error.is_none());
}
