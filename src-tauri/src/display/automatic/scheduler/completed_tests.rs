use super::scheduler::Scheduler;
use crate::hal::power_events::{Event, Source};
use ccc_core::{oem_refresh::Trigger, oem_supply_mode::Trigger as ModeTrigger};

fn ready() -> Scheduler {
    let mut scheduler = Scheduler::default();
    scheduler.ready().unwrap();
    scheduler.configure(2, true, Trigger::Startup, 0).unwrap();
    scheduler.observe(Event::Source(Source::Ac), 0).unwrap();
    scheduler.take(1_000).unwrap();
    scheduler
}
#[test]
fn a_completion_keeps_the_actual_previous_mode_and_oem_trigger() {
    for (mode_trigger, dc_mode, refresh_trigger) in [
        (ModeTrigger::Startup, true, Trigger::DcModeChange),
        (ModeTrigger::Startup, false, Trigger::ModeChange),
        (ModeTrigger::SupplyChange, true, Trigger::DcModeChange),
        (ModeTrigger::SupplyChange, false, Trigger::DcModeChange),
        (ModeTrigger::Resume, true, Trigger::Resume),
        (ModeTrigger::Resume, false, Trigger::ModeChange),
    ] {
        let mut scheduler = ready();
        scheduler
            .completed(1, 3, true, mode_trigger, dc_mode, 2_000)
            .unwrap();
        assert!(scheduler.take(2_999).is_none());
        let work = scheduler.take(3_000).unwrap();
        assert_eq!(
            (work.trigger, work.previous_mode),
            (refresh_trigger, Some(3))
        );
        assert!(work.completion.is_some());
        assert!(scheduler.take(9_000).is_none());
        scheduler.acknowledge(work);
        assert!(scheduler.take(9_000).is_none());
    }
}
#[test]
fn completion_before_the_other_listener_does_not_get_lost_when_supply_reconciles() {
    let mut scheduler = ready();
    scheduler
        .completed(1, 2, false, ModeTrigger::SupplyChange, true, 2_000)
        .unwrap();
    scheduler.read_supply(Source::Battery, 2_100).unwrap();
    assert!(scheduler.take(8_000).is_none());
    scheduler
        .observe(Event::Source(Source::Battery), 8_100)
        .unwrap();
    scheduler.read_supply(Source::Battery, 8_200).unwrap();
    let work = scheduler.take(9_200).unwrap();
    assert_eq!(
        (work.trigger, work.previous_mode),
        (Trigger::DcModeChange, Some(2))
    );
}
#[test]
fn notification_before_mode_completion_does_not_replay_an_ordinary_supply_work() {
    let mut scheduler = ready();
    scheduler
        .observe(Event::Source(Source::Battery), 2_000)
        .unwrap();
    scheduler
        .completed(1, 2, false, ModeTrigger::SupplyChange, true, 2_100)
        .unwrap();
    scheduler.mode(1, 2_200).unwrap();
    let work = scheduler.take(3_100).unwrap();
    assert_eq!(work.trigger, Trigger::DcModeChange);
    scheduler.acknowledge(work);
    assert!(scheduler.take(9_000).is_none());
}
#[test]
fn a_rejected_in_flight_handoff_remains_available_until_successfully_acknowledged() {
    let mut scheduler = ready();
    scheduler
        .completed(1, 2, true, ModeTrigger::Startup, true, 2_000)
        .unwrap();
    let old = scheduler.take(3_000).unwrap();
    scheduler
        .observe(Event::Source(Source::Unknown), 3_001)
        .unwrap();
    assert!(!scheduler.permits(old.epoch));
    scheduler.observe(Event::Source(Source::Ac), 4_000).unwrap();
    scheduler.read_supply(Source::Ac, 4_001).unwrap();
    let retry = scheduler.take(5_001).unwrap();
    assert_eq!(retry.completion, old.completion);
    assert_ne!(retry.epoch, old.epoch);
    scheduler.acknowledge(retry);
    assert!(scheduler.take(10_000).is_none());
}
#[test]
fn a_late_old_acknowledgement_does_not_consume_a_new_mode_completion() {
    let mut scheduler = ready();
    scheduler
        .completed(1, 2, true, ModeTrigger::Startup, true, 2_000)
        .unwrap();
    let old = scheduler.take(3_000).unwrap();
    scheduler
        .completed(3, 1, true, ModeTrigger::SupplyChange, true, 3_001)
        .unwrap();
    scheduler.acknowledge(old);
    let new = scheduler.take(4_001).unwrap();
    assert_ne!(new.completion, old.completion);
    assert_eq!(new.previous_mode, Some(1));
}
#[test]
fn a_completion_can_arrive_before_listener_configuration_but_sleep_and_stop_discard_it() {
    let mut scheduler = Scheduler::default();
    scheduler
        .completed(1, 2, false, ModeTrigger::Startup, true, 0)
        .unwrap();
    scheduler.ready().unwrap();
    scheduler.configure(1, false, Trigger::Startup, 10).unwrap();
    let work = scheduler.take(1_000).unwrap();
    assert_eq!(work.previous_mode, Some(2));
    scheduler.observe(Event::Suspend, 1_001).unwrap();
    scheduler.observe(Event::Resume, 2_000).unwrap();
    scheduler.read_supply(Source::Battery, 2_001).unwrap();
    scheduler.mode(1, 2_001).unwrap();
    assert!(scheduler.take(9_000).is_none());
    scheduler.stop().unwrap();
    assert!(
        scheduler
            .completed(1, 2, false, ModeTrigger::Startup, true, 10_000)
            .is_err()
    );
}
#[test]
fn a_real_external_mode_change_replaces_the_pending_completion() {
    let mut scheduler = ready();
    scheduler
        .completed(1, 2, true, ModeTrigger::Startup, true, 2_000)
        .unwrap();
    scheduler.mode(3, 2_100).unwrap();
    let work = scheduler.take(3_100).unwrap();
    assert_eq!(work.trigger, Trigger::ModeChange);
    assert!(work.completion.is_none());
    assert_eq!(work.previous_mode, Some(1));
}
