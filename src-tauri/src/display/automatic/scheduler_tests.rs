use super::scheduler::Scheduler;
use crate::hal::power_events::{Event, Source};
use ccc_core::oem_refresh::Trigger;

#[test]
fn mode_and_supply_changes_invalidate_old_work_and_wait_for_stability() {
    let mut scheduler = Scheduler::default();
    scheduler.ready().unwrap();
    scheduler.configure(1, false, Trigger::Startup, 0).unwrap();
    assert!(scheduler.take(999).is_none());
    let startup = scheduler.take(1000).unwrap();
    assert_eq!(startup.previous_mode, Some(1));
    assert!(scheduler.permits(startup.epoch));
    scheduler.mode(2, 1100).unwrap();
    assert!(!scheduler.permits(startup.epoch));
    assert!(scheduler.take(2099).is_none());
    let change = scheduler.take(2100).unwrap();
    assert_eq!(
        (change.trigger, change.previous_mode),
        (Trigger::ModeChange, Some(1))
    );
    scheduler.supply(Source::Ac, 2200).unwrap();
    assert!(!scheduler.permits(change.epoch));
    assert_eq!(scheduler.take(3200).unwrap().trigger, Trigger::SupplyChange);
}

#[test]
fn unknown_supply_sleep_listener_loss_and_disable_revoke_trials() {
    let mut scheduler = Scheduler::default();
    scheduler.ready().unwrap();
    scheduler
        .configure(1, false, Trigger::UserChange, 0)
        .unwrap();
    let work = scheduler.take(1000).unwrap();
    scheduler
        .observe(Event::Source(Source::Unknown), 1001)
        .unwrap();
    assert!(!scheduler.permits(work.epoch) && scheduler.take(3000).is_none());
    scheduler
        .observe(Event::Source(Source::Battery), 3001)
        .unwrap();
    let next = scheduler.take(4001).unwrap();
    scheduler.observe(Event::Suspend, 4002).unwrap();
    assert!(!scheduler.permits(next.epoch));
    scheduler.observe(Event::Resume, 5000).unwrap();
    scheduler.supply(Source::Ac, 5001).unwrap();
    scheduler.mode(2, 5001).unwrap();
    assert!(scheduler.take(7000).is_none());
    scheduler.fail().unwrap();
    assert!(!scheduler.online);
    scheduler.disable().unwrap();
    assert!(!scheduler.enabled);
}

#[test]
fn simultaneous_supply_and_mode_changes_both_remain_scheduled() {
    let mut scheduler = Scheduler::default();
    scheduler.ready().unwrap();
    scheduler.configure(1, false, Trigger::Startup, 0).unwrap();
    scheduler.take(1000).unwrap();
    scheduler.observe(Event::Source(Source::Ac), 1100).unwrap();
    scheduler.mode(3, 1200).unwrap();
    let first = scheduler.take(2200).unwrap();
    let second = scheduler.take(2200).unwrap();
    assert_eq!(
        (first.trigger, second.trigger),
        (Trigger::SupplyChange, Trigger::ModeChange)
    );
    assert_eq!(second.previous_mode, Some(1));
}

#[test]
fn ups_or_inconsistent_notifications_cannot_be_overridden_by_a_poll() {
    let mut scheduler = Scheduler::default();
    scheduler.ready().unwrap();
    scheduler.configure(1, true, Trigger::Startup, 0).unwrap();
    let initial = scheduler.take(1000).unwrap();
    scheduler.observe(Event::Source(Source::Ups), 1001).unwrap();
    scheduler.read_supply(Source::Ac, 2000).unwrap();
    assert!(!scheduler.permits(initial.epoch));
    assert!(scheduler.take(4000).is_none());
    scheduler
        .observe(Event::Source(Source::Battery), 4001)
        .unwrap();
    scheduler.read_supply(Source::Ac, 4002).unwrap();
    assert!(scheduler.take(6000).is_none());
    scheduler.read_supply(Source::Battery, 6001).unwrap();
    assert_eq!(scheduler.take(7001).unwrap().trigger, Trigger::SupplyChange);
}
