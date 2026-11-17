use super::*;

fn enabled() -> Scheduler {
    let mut scheduler = Scheduler::default();
    scheduler.ready().unwrap();
    scheduler
        .configure(Source::Ac, Trigger::Startup, 100)
        .unwrap();
    scheduler
}

#[test]
fn startup_and_source_changes_wait_for_stable_matching_system_reads() {
    let mut scheduler = enabled();
    assert_eq!(scheduler.take(1_099), None);
    assert_eq!(scheduler.take(1_100).unwrap().trigger, Trigger::Startup);
    let old = scheduler.ticket();
    scheduler
        .observe(Event::Source(Source::Battery), 2_000)
        .unwrap();
    assert!(!scheduler.permits(old, Source::Ac));
    scheduler.read_source(Source::Ac, 2_500).unwrap();
    assert!(scheduler.take(10_000).is_none());
    scheduler.read_source(Source::Battery, 3_000).unwrap();
    assert!(scheduler.take(3_999).is_none());
    let work = scheduler.take(4_000).unwrap();
    assert_eq!(
        (work.source, work.trigger),
        (Source::Battery, Trigger::SupplyChange)
    );
    assert!(scheduler.permits(work.epoch, Source::Battery));
}

#[test]
fn unknown_ups_or_conflicting_notifications_never_gain_write_permission() {
    let mut scheduler = enabled();
    let ticket = scheduler.ticket();
    for (time, source) in [(300, Source::Ups), (400, Source::Unknown)] {
        scheduler.observe(Event::Source(source), time).unwrap();
        scheduler.read_source(Source::Ac, time).unwrap();
        assert!(!scheduler.permits(ticket, Source::Ac));
        assert!(scheduler.take(time + 10_000).is_none());
    }
    scheduler.observe(Event::Source(Source::Ac), 500).unwrap();
    scheduler.read_source(Source::Ac, 500).unwrap();
    assert_eq!(scheduler.take(1_500).unwrap().trigger, Trigger::Startup);
}

#[test]
fn wake_is_not_mistaken_for_supply_change_and_invalidates_inflight_work() {
    let mut scheduler = enabled();
    let work = scheduler.take(1_100).unwrap();
    scheduler.observe(Event::Suspend, 2_000).unwrap();
    assert!(!scheduler.permits(work.epoch, work.source));
    scheduler
        .observe(Event::Source(Source::Battery), 2_100)
        .unwrap();
    scheduler.observe(Event::Resume, 2_200).unwrap();
    scheduler.read_source(Source::Battery, 2_300).unwrap();
    let resumed = scheduler.take(3_300).unwrap();
    assert_eq!(
        (resumed.trigger, resumed.source),
        (Trigger::Resume, Source::Battery)
    );
}

#[test]
fn stopping_and_listener_failure_revoke_tickets_without_output() {
    let mut scheduler = enabled();
    let ticket = scheduler.ticket();
    scheduler.fail().unwrap();
    assert!(!scheduler.permits_setup(ticket));
    assert!(!scheduler.permits(ticket, Source::Ac));
    assert!(scheduler.take(10_000).is_none());
    assert!(
        scheduler
            .configure(Source::Ac, Trigger::Startup, 0)
            .is_err()
    );
    let mut scheduler = enabled();
    scheduler.stop().unwrap();
    assert!(scheduler.take(10_000).is_none());
    assert!(
        scheduler
            .configure(Source::Ac, Trigger::Startup, 0)
            .is_err()
    );
}

#[test]
fn returning_to_the_previous_source_discards_a_transient_mismatch() {
    let mut scheduler = enabled();
    scheduler.take(1_100).unwrap();
    scheduler
        .observe(Event::Source(Source::Battery), 2_000)
        .unwrap();
    scheduler.read_source(Source::Ac, 2_100).unwrap();
    scheduler.observe(Event::Source(Source::Ac), 2_200).unwrap();
    scheduler.read_source(Source::Ac, 2_300).unwrap();
    assert!(scheduler.take(5_000).is_none());
}
