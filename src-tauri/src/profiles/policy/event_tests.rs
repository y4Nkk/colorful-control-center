use super::*;

fn scheduler(resume: bool) -> Scheduler {
    let mut s = Scheduler::default();
    s.observe(Event::Source(Source::Ac), 0).unwrap();
    s.configure(true, [true, true, resume]).unwrap();
    s
}

#[test]
fn exhausted_event_generation_revokes_existing_permission_instead_of_wrapping() {
    let mut s = scheduler(true);
    s.epoch = u64::MAX;
    assert!(s.permits(u64::MAX));
    assert!(s.observe(Event::Source(Source::Battery), 1).is_err());
    assert!(!s.permits(u64::MAX));
    assert!(s.take(u64::MAX).is_none());
}

#[test]
fn first_supply_baseline_does_not_erase_a_real_resume_already_observed() {
    let mut s = Scheduler::default();
    s.configure(true, [true; 3]).unwrap();
    s.observe(Event::Suspend, 1).unwrap();
    s.observe(Event::Resume, 2).unwrap();
    s.observe(Event::Source(Source::Ac), 3).unwrap();
    assert_eq!(s.take(3 + STABLE_MS).unwrap().trigger, Trigger::Resume);
}

#[test]
fn initial_enable_and_duplicate_notifications_never_apply_current_supply() {
    let mut s = Scheduler::default();
    s.configure(true, [true; 3]).unwrap();
    s.observe(Event::Source(Source::Ac), 1).unwrap();
    let epoch = s.epoch;
    s.observe(Event::Source(Source::Ac), 2).unwrap();
    s.observe(Event::DisplayAwake(true), 3).unwrap();
    s.observe(Event::Resume, 4).unwrap();
    assert!(s.take(u64::MAX).is_none());
    assert_eq!(s.epoch, epoch);
    s.configure(false, [true; 3]).unwrap();
    s.observe(Event::Source(Source::Battery), 5).unwrap();
    s.configure(true, [true; 3]).unwrap();
    assert!(s.take(u64::MAX).is_none());
}

#[test]
fn supply_bounce_invalidates_old_permission_and_only_applies_final_stable_source() {
    let mut s = scheduler(false);
    let epoch = s.epoch;
    s.observe(Event::Source(Source::Battery), 10).unwrap();
    assert!(!s.permits(epoch));
    assert!(s.take(10 + STABLE_MS - 1).is_none());
    s.observe(Event::Source(Source::Ac), 100).unwrap();
    s.observe(Event::Source(Source::Ac), 200).unwrap();
    assert!(s.take(100 + STABLE_MS - 1).is_none());
    let pending = s.take(100 + STABLE_MS).unwrap();
    assert_eq!(pending.trigger, Trigger::Ac);
    assert!(s.permits(pending.epoch));
    assert!(s.take(u64::MAX).is_none());
    s.configure(false, [true; 3]).unwrap();
    assert!(!s.permits(pending.epoch));
}

#[test]
fn ups_unknown_and_unselected_supply_never_become_battery_actions() {
    for source in [Source::Ups, Source::Unknown] {
        let mut s = scheduler(false);
        s.observe(Event::Source(Source::Battery), 1).unwrap();
        let epoch = s.epoch;
        s.observe(Event::Source(source), 2).unwrap();
        assert!(!s.permits(epoch));
        assert!(s.take(u64::MAX).is_none());
    }
    let mut s = scheduler(false);
    s.configure(true, [true, false, false]).unwrap();
    s.observe(Event::Source(Source::Battery), 1).unwrap();
    assert!(s.take(u64::MAX).is_none());
}

#[test]
fn real_suspend_clears_queued_work_and_resume_wins_over_sleep_supply_changes() {
    let mut s = scheduler(true);
    s.observe(Event::Source(Source::Battery), 1).unwrap();
    s.observe(Event::Suspend, 2).unwrap();
    assert!(s.take(u64::MAX).is_none());
    s.observe(Event::Source(Source::Ac), 3).unwrap();
    assert!(s.take(u64::MAX).is_none());
    s.observe(Event::Resume, 4).unwrap();
    s.observe(Event::Source(Source::Battery), 5).unwrap();
    s.observe(Event::Resume, 6).unwrap();
    assert!(s.take(5 + STABLE_MS - 1).is_none());
    assert_eq!(s.take(5 + STABLE_MS).unwrap().trigger, Trigger::Resume);
    let epoch = s.epoch;
    s.observe(Event::Resume, 7).unwrap();
    assert_eq!(s.epoch, epoch);
    assert!(s.take(u64::MAX).is_none());
}

#[test]
fn without_resume_rule_latest_supply_during_sleep_is_retained_and_redebounced() {
    let mut s = scheduler(false);
    s.observe(Event::Suspend, 1).unwrap();
    s.observe(Event::Source(Source::Battery), 2).unwrap();
    s.observe(Event::Source(Source::Ac), 3).unwrap();
    s.observe(Event::Source(Source::Battery), 4).unwrap();
    s.observe(Event::Resume, 5_000).unwrap();
    assert!(s.take(5_000 + STABLE_MS - 1).is_none());
    assert_eq!(s.take(5_000 + STABLE_MS).unwrap().trigger, Trigger::Battery);
}

#[test]
fn modern_standby_and_system_notifications_share_one_sleep_cycle() {
    let mut s = scheduler(true);
    s.observe(Event::DisplayAwake(true), 1).unwrap();
    s.observe(Event::DisplayAwake(false), 2).unwrap();
    s.observe(Event::Suspend, 3).unwrap();
    s.observe(Event::Source(Source::Battery), 4).unwrap();
    s.observe(Event::Resume, 5).unwrap();
    assert!(s.sleeping());
    assert!(s.take(u64::MAX).is_none());
    s.observe(Event::DisplayAwake(true), 6).unwrap();
    assert!(!s.sleeping());
    assert_eq!(s.take(6 + STABLE_MS).unwrap().trigger, Trigger::Resume);
    s.observe(Event::DisplayAwake(true), 7).unwrap();
    s.observe(Event::Resume, 8).unwrap();
    assert!(s.take(u64::MAX).is_none());
}

#[test]
fn initial_dark_display_seeds_state_without_false_sleep_or_resume_work() {
    let mut s = scheduler(true);
    s.observe(Event::DisplayAwake(false), 1).unwrap();
    assert!(s.sleeping() && s.take(u64::MAX).is_none());
    s.observe(Event::DisplayAwake(false), 2).unwrap();
    assert!(s.take(u64::MAX).is_none());
    s.observe(Event::DisplayAwake(true), 3).unwrap();
    assert_eq!(s.take(3 + STABLE_MS).unwrap().trigger, Trigger::Resume);
}

#[test]
fn policy_changes_drop_backlog_and_invalidate_taken_work_without_replaying_source() {
    let mut s = scheduler(true);
    s.observe(Event::Source(Source::Battery), 1).unwrap();
    let pending = s.take(u64::MAX).unwrap();
    s.configure(true, [true, false, true]).unwrap();
    assert!(!s.permits(pending.epoch));
    assert!(s.take(u64::MAX).is_none());
    s.observe(Event::Source(Source::Ac), 2).unwrap();
    s.configure(false, [true; 3]).unwrap();
    assert!(s.take(u64::MAX).is_none());
}
