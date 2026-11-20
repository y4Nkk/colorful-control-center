use super::super::worker::ApplicationIo;
use super::fixture::Harness;
use crate::{
    hal::power_events::{Event, Source},
    profiles::application::Origin,
};

#[test]
fn repeated_approved_profile_modes_save_the_preference_without_invalidating_future_rules() {
    let h = Harness::new(false);
    h.f.device.0.lock().unwrap().pages[1][16] = 3;
    for (name, mode) in [("插电", 2), ("电池", 1), ("唤醒", 3)] {
        let mut file = h.file(name);
        file.settings.oem_power_mode = Some(mode);
        h.write(&file);
    }
    let mut request = h.request(true);
    for selection in [&mut request.ac, &mut request.battery, &mut request.resume]
        .into_iter()
        .flatten()
    {
        selection.mode =
            crate::profiles::preview_authorization(&h.context(), &selection.file.settings).unwrap();
        assert!(selection.mode.as_ref().unwrap().target.ac_mode.is_some());
    }
    h.policy
        .save(&h.context(), &h.profiles(), &request)
        .unwrap();
    for (source, now, mode) in [
        (Source::Battery, 1, 1),
        (Source::Ac, 2_000, 2),
        (Source::Battery, 4_000, 1),
    ] {
        h.observe(Event::Source(source), now);
        h.tick(now + 1_000).unwrap();
        assert!(h.last().unwrap().targets_applied);
        let data = h.f.device.0.lock().unwrap();
        assert_eq!(data.pages[1][1], mode);
        assert_eq!(data.pages[1][16], mode);
    }
    h.policy.restore(&h.f.hal, &h.record()).unwrap();
    let stored = h
        .f
        .hal
        .recovery()
        .unwrap()
        .oem_mode
        .clone()
        .unwrap()
        .stored();
    assert_eq!(
        stored.recovery_preview(&stored).unwrap().initial_ac_mode,
        Some(3)
    );
}

#[test]
fn actual_application_pipeline_uses_latest_stable_supply_and_never_initial_state() {
    let h = Harness::new(false);
    h.enable(false);
    h.tick(2_000).unwrap();
    assert!(h.last().is_none());
    h.observe(Event::Source(Source::Battery), 3_000);
    h.tick(3_999).unwrap();
    assert!(h.last().is_none());
    h.observe(Event::Source(Source::Ac), 3_500);
    h.tick(4_500).unwrap();
    assert_eq!(h.watts(), 75);
    let report = h.last().unwrap();
    assert_eq!(report.origin, Origin::Ac);
    assert!(report.targets_applied);
    h.tick(9_000).unwrap();
    assert_eq!(h.last().unwrap().id, report.id);
}

#[test]
fn wake_priority_and_no_resume_rule_use_the_same_application_transactions() {
    let h = Harness::new(false);
    h.enable(true);
    h.observe(Event::Suspend, 1);
    h.observe(Event::Source(Source::Battery), 2);
    h.tick(2_000).unwrap();
    assert!(h.last().is_none());
    h.observe(Event::Resume, 3_000);
    h.tick(4_000).unwrap();
    assert_eq!(h.last().unwrap().origin, Origin::Resume);
    assert_eq!(h.watts(), 65);
    h.enable(false);
    h.observe(Event::Suspend, 5_000);
    h.observe(Event::Source(Source::Ac), 5_100);
    h.observe(Event::Resume, 6_000);
    h.tick(7_000).unwrap();
    assert_eq!(h.last().unwrap().origin, Origin::Ac);
    assert_eq!(h.watts(), 75);
}

#[test]
fn pause_and_source_change_cancel_only_owned_trial_before_other_outputs() {
    let h = Harness::new(true);
    h.enable(false);
    h.observe(Event::Source(Source::Battery), 1);
    h.tick(1_001).unwrap();
    let first = h.last().unwrap();
    assert!(first.pending.is_some());
    assert_eq!(h.watts(), 65);
    h.policy.pause().unwrap();
    h.tick(1_002).unwrap();
    assert!(h.last().unwrap().pending.is_none());
    assert_eq!(h.watts(), 65);
    h.enable(false);
    h.observe(Event::Source(Source::Ac), 2_000);
    h.observe(Event::Source(Source::Battery), 2_001);
    h.tick(3_001).unwrap();
    assert!(h.last().unwrap().pending.is_some());
    h.observe(Event::Source(Source::Ac), 3_002);
    h.tick(3_003).unwrap();
    assert!(h.last().unwrap().pending.is_none());
    assert_eq!(h.watts(), 65);
}

#[test]
fn edited_bound_file_cancels_owned_trial_and_rejects_old_confirmation() {
    let h = Harness::new(true);
    h.enable(false);
    h.observe(Event::Source(Source::Battery), 1);
    h.tick(1_001).unwrap();
    let mut file = h.file("电池");
    file.settings.pl1_w = Some(60.0);
    h.write(&file);
    h.tick(1_002).unwrap();
    assert!(h.last().unwrap().pending.is_none());
    assert_eq!(h.watts(), 65);
    h.observe(Event::Source(Source::Ac), 2_000);
    h.observe(Event::Source(Source::Battery), 2_001);
    assert!(h.tick(3_001).is_err());
    assert_eq!(h.watts(), 65);
}

#[test]
fn display_keep_retains_automatic_origin_and_listener_failure_blocks_old_keep() {
    let h = Harness::new(true);
    h.enable(false);
    h.observe(Event::Source(Source::Battery), 1);
    h.tick(1_001).unwrap();
    let report = h.keep(h.last().unwrap().id).unwrap();
    assert_eq!(report.origin, Origin::Battery);
    assert!(report.targets_applied && report.pending.is_none());
    assert_eq!(h.watts(), 55);
    h.observe(Event::Source(Source::Ac), 2_000);
    h.tick(3_000).unwrap();
    assert_eq!(h.watts(), 75);
    h.f.display.io.0.lock().unwrap().displays[0]
        .current
        .frequency = 60;
    h.observe(Event::Source(Source::Battery), 4_000);
    h.tick(5_000).unwrap();
    h.policy.shared.failed("隔离监听退出".into());
    let report = h.keep(h.last().unwrap().id).unwrap();
    assert!(!report.targets_applied && report.pending.is_none());
    assert_eq!(h.watts(), 75);
}

#[test]
fn manual_trials_are_never_cancelled_by_pause_or_worker_finish() {
    let h = Harness::new(true);
    h.enable(false);
    let manual = h.begin_manual();
    h.observe(Event::Source(Source::Battery), 1);
    h.tick(2_000).unwrap();
    h.policy.pause().unwrap();
    h.tick(2_001).unwrap();
    h.finish().unwrap();
    assert_eq!(h.last().unwrap().id, manual.id);
    assert!(h.last().unwrap().pending.is_some());
    h.cancel(manual.id).unwrap();
}

#[test]
fn ownership_does_not_transfer_when_user_cancelled_then_started_manual_trial() {
    let h = Harness::new(true);
    h.enable(false);
    h.observe(Event::Source(Source::Battery), 1);
    h.tick(1_001).unwrap();
    h.cancel(h.last().unwrap().id).unwrap();
    let manual = h.begin_manual();
    h.policy.pause().unwrap();
    h.finish().unwrap();
    assert_eq!(h.last().unwrap().id, manual.id);
    assert!(h.last().unwrap().pending.is_some());
    h.cancel(manual.id).unwrap();
}
