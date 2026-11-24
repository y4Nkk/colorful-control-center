use super::*;
use crate::display::profile::{Review, Target};

fn group() -> (Controller, Fake, Target) {
    let (controller, io) = setup();
    let mut second = mode();
    second.device = "DISPLAY2".into();
    second.identity = "panel-b".into();
    second.x = 1920;
    io.0.lock().unwrap().modes.push(Display {
        current: second,
        frequencies: vec![60, 144],
        primary: false,
    });
    let mut target = controller.capture().unwrap().unwrap();
    for display in &mut target.displays {
        display.frequency = 144;
    }
    (controller, io, target)
}
fn begin(controller: &Controller, target: &Target) -> Review {
    let review = controller.preview_profile(target, None).unwrap();
    assert!(controller.begin_profile(&review, 42, 0).unwrap());
    review
}

#[test]
fn capture_and_preview_do_not_write() {
    let (controller, io, target) = group();
    let captured = controller.capture().unwrap().unwrap();
    assert!(captured.displays.iter().all(|mode| mode.frequency == 60));
    let review = controller.preview_profile(&target, None).unwrap();
    assert_eq!(review.changes().len(), 2);
    assert_eq!(io.0.lock().unwrap().writes, 0);
    io.0.lock().unwrap().modes.clear();
    assert!(controller.capture().unwrap().is_none());
}

#[test]
fn group_trial_blocks_independent_writes_and_requires_group_owner() {
    let (controller, io, target) = group();
    begin(&controller, &target);
    let pending = controller.profile_pending(42, 1000).unwrap().unwrap();
    assert_eq!(pending.targets.len(), 2);
    assert_eq!(pending.remaining_ms, Some(14_000));
    assert!(controller.keep(1, &target.displays[0], 1).is_err());
    assert!(controller.cancel(1).is_err());
    assert!(controller.keep_profile(41, 1).is_err());
    assert!(controller.apply(&target.displays[0], 60, 1).is_err());
    assert!(controller.capture().is_err());
    assert_eq!(io.0.lock().unwrap().writes, 2);
}

#[test]
fn retained_trial_keeps_current_targets_and_remains_reversible_until_commit() {
    let (controller, io, target) = group();
    begin(&controller, &target);
    controller.keep_profile(42, 100).unwrap();
    assert!(controller.trial_pending().unwrap());
    assert!(
        controller
            .profile_pending(42, 100)
            .unwrap()
            .unwrap()
            .remaining_ms
            .is_none()
    );
    assert!(
        controller
            .preview_profile(&target, Some(42))
            .unwrap()
            .changes()
            .is_empty()
    );
    assert!(controller.preview_profile(&target, None).is_err());
    assert!(controller.release_profile(41).is_err());
    assert_eq!(io.0.lock().unwrap().writes, 2);
    controller.cancel_profile(42).unwrap();
    assert!(!controller.trial_pending().unwrap());
    assert!(
        io.list()
            .unwrap()
            .iter()
            .all(|display| display.current.frequency == 60)
    );
}

#[test]
fn successful_release_preserves_targets() {
    let (controller, io, target) = group();
    begin(&controller, &target);
    controller.keep_profile(42, 100).unwrap();
    controller.release_profile(42).unwrap();
    assert!(!controller.trial_pending().unwrap());
    assert_eq!(io.0.lock().unwrap().writes, 2);
    assert!(
        io.list()
            .unwrap()
            .iter()
            .all(|display| display.current.frequency == 144)
    );
}

#[test]
fn timeout_restores_group_and_does_not_become_retained_confirmation() {
    let (controller, io, target) = group();
    begin(&controller, &target);
    assert!(controller.keep_profile(42, 15_000).is_err());
    controller.tick(15_000).unwrap();
    assert!(controller.profile_pending(42, 15_000).unwrap().is_none());
    assert!(controller.keep_profile(42, 15_001).is_err());
    assert!(
        io.list()
            .unwrap()
            .iter()
            .all(|display| display.current.frequency == 60)
    );
}

#[test]
fn changed_geometry_candidates_and_stale_review_reject_before_any_write() {
    let (controller, io, mut target) = group();
    target.displays[1].x += 1;
    assert!(controller.preview_profile(&target, None).is_err());
    target.displays[1].x -= 1;
    let review = controller.preview_profile(&target, None).unwrap();
    io.0.lock().unwrap().modes[1].frequencies = vec![60];
    assert!(controller.preview_profile(&target, None).is_err());
    assert!(controller.begin_profile(&review, 42, 0).is_err());
    io.0.lock().unwrap().modes[1].frequencies = vec![60, 144];
    io.0.lock().unwrap().modes[1].current.frequency = 144;
    assert!(controller.begin_profile(&review, 42, 0).is_err());
    assert_eq!(io.0.lock().unwrap().writes, 0);
}

#[test]
fn no_change_trial_does_not_start_guard() {
    let (_, io, _) = group();
    let controller = Controller::with_guards(io.clone(), FaultFactory(GuardFault::Start));
    let target = controller.capture().unwrap().unwrap();
    let review = controller.preview_profile(&target, None).unwrap();
    assert!(!controller.begin_profile(&review, 42, 0).unwrap());
    assert!(controller.profile_pending(42, 0).unwrap().is_none());
    assert_eq!(io.0.lock().unwrap().writes, 0);
}

#[test]
fn cancel_uses_this_application_before_state() {
    let (controller, io, mut target) = group();
    controller.apply(&mode(), 144, 0).unwrap();
    let current = io.list().unwrap()[0].current.clone();
    controller.keep(1, &current, 1).unwrap();
    target.displays[0].frequency = 60;
    begin(&controller, &target);
    controller.keep_profile(42, 2).unwrap();
    controller.cancel_profile(42).unwrap();
    assert_eq!(io.list().unwrap()[0].current.frequency, 144);
    assert_eq!(io.list().unwrap()[1].current.frequency, 60);
}

#[test]
fn partial_failure_restores_all_attempted_displays_and_retains_failed_recovery() {
    let (controller, io, target) = group();
    io.0.lock().unwrap().fail = vec![2, 3];
    let review = controller.preview_profile(&target, None).unwrap();
    assert!(controller.begin_profile(&review, 42, 0).is_err());
    assert!(controller.profile_pending(42, 0).unwrap().is_some());
    assert_eq!(io.0.lock().unwrap().writes, 4);
    controller.cancel_profile(42).unwrap();
    assert!(controller.profile_pending(42, 0).unwrap().is_none());
    assert!(
        io.list()
            .unwrap()
            .iter()
            .all(|display| display.current.frequency == 60)
    );
}

#[test]
fn external_change_after_keep_blocks_further_preflight_and_preserves_external_mode() {
    let (controller, io, target) = group();
    begin(&controller, &target);
    controller.keep_profile(42, 100).unwrap();
    io.0.lock().unwrap().modes[1].current.frequency = 120;
    assert!(controller.preview_profile(&target, Some(42)).is_err());
    assert!(controller.cancel_profile(42).is_err());
    assert_eq!(io.list().unwrap()[0].current.frequency, 60);
    assert_eq!(io.list().unwrap()[1].current.frequency, 120);
    assert!(controller.profile_pending(42, 100).unwrap().is_some());
    io.0.lock().unwrap().modes[1].current.frequency = 144;
    controller.cancel_profile(42).unwrap();
}

#[test]
fn display_target_and_review_reject_duplicate_unknown_and_geometry_inputs() {
    let (_, _, target) = group();
    let mut duplicate = target.clone();
    duplicate.displays[1] = duplicate.displays[0].clone();
    assert!(duplicate.validate().is_err());
    let mut json = serde_json::to_value(target).unwrap();
    json["fallback"] = serde_json::json!(true);
    assert!(serde_json::from_value::<Target>(json).is_err());
}
