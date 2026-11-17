use super::tests::*;
use super::*;

#[test]
fn capture_preserves_accepted_choice_instead_of_boot_report() {
    let (_root, store, io) = setup();
    let before = preview(&io, &store).unwrap().unwrap();
    assert_eq!(profile::capture(&before).unwrap().mode, 3);
    apply(&io, &store, &io.request(&store, 2)).unwrap();
    let accepted = preview(&io, &store).unwrap().unwrap();
    assert_eq!(accepted.observed.mode, 3);
    assert_eq!(profile::capture(&accepted).unwrap().mode, 2);
    io.boot_to('2', 3);
    assert_eq!(
        profile::capture(&preview(&io, &store).unwrap().unwrap())
            .unwrap()
            .mode,
        3
    );
}

#[test]
fn uncertain_choice_is_not_captured_or_overwritten_by_new_target() {
    let (_root, store, io) = setup();
    *io.fail_send.borrow_mut() = vec![1, 2];
    assert!(apply(&io, &store, &io.request(&store, 2)).is_err());
    let before = preview(&io, &store).unwrap().unwrap();
    assert!(profile::capture(&before).is_err());
    let count = io.sent.borrow().len();
    assert!(apply(&io, &store, &io.request(&store, 4)).is_err());
    assert_eq!(io.sent.borrow().len(), count);
}

#[test]
fn completed_plan_restores_previous_choice_and_keeps_first_baseline() {
    let (_root, store, io) = setup();
    apply(&io, &store, &io.request(&store, 2)).unwrap();
    let mut plan =
        transaction::Plan::new(preview(&io, &store).unwrap().unwrap(), 4, false).unwrap();
    plan.execute(&io, &store).unwrap();
    assert_eq!(verification(&io, &store), Verification::PendingReboot);
    plan.rollback(&io, &store).unwrap();
    let record = store.load().unwrap().record.unwrap();
    assert_eq!((record.initial_mode, record.target), (3, 2));
    assert_eq!(*io.sent.borrow(), vec![2, 4, 2]);
    plan.rollback(&io, &store).unwrap();
    assert_eq!(io.sent.borrow().len(), 3);
    assert_eq!(*io.reboots.borrow(), 0);
}

#[test]
fn failed_compensation_can_retry_without_discarding_ownership() {
    let (_root, store, io) = setup();
    let mut plan =
        transaction::Plan::new(preview(&io, &store).unwrap().unwrap(), 2, false).unwrap();
    plan.execute(&io, &store).unwrap();
    *io.fail_send.borrow_mut() = vec![2];
    assert!(plan.rollback(&io, &store).is_err());
    plan.rollback(&io, &store).unwrap();
    assert_eq!(store.load().unwrap().record.unwrap().target, 3);
    assert_eq!(*io.sent.borrow(), vec![2, 3, 3]);
}

#[test]
fn rollback_does_not_overwrite_foreign_record_boot_or_mode() {
    for change in 0..3 {
        let (_root, store, io) = setup();
        let mut plan =
            transaction::Plan::new(preview(&io, &store).unwrap().unwrap(), 2, false).unwrap();
        plan.execute(&io, &store).unwrap();
        match change {
            0 => {
                apply(&io, &store, &io.request(&store, 4)).unwrap();
            }
            1 => io.boot_to('2', 3),
            _ => io.observed.borrow_mut().as_mut().unwrap().mode = 4,
        }
        let count = io.sent.borrow().len();
        assert!(plan.rollback(&io, &store).is_err());
        assert_eq!(io.sent.borrow().len(), count);
    }
}

#[test]
fn repeated_accepted_pending_target_does_not_resend_or_reboot() {
    let (_root, store, io) = setup();
    apply(&io, &store, &io.request(&store, 2)).unwrap();
    apply(&io, &store, &io.request(&store, 2)).unwrap();
    assert_eq!(*io.sent.borrow(), vec![2]);
    assert_eq!(*io.reboots.borrow(), 0);
}

#[test]
fn profile_target_rejects_unknown_modes_candidates_owner_and_fields() {
    let base = profile::Target {
        owner: "a".repeat(64),
        available_mask: 15,
        mode: 3,
    };
    for (mode, mask, owner) in [
        (0, 15, base.owner.clone()),
        (2, 1, base.owner.clone()),
        (3, 255, base.owner.clone()),
        (3, 15, "A".repeat(64)),
    ] {
        assert!(
            profile::Target {
                mode,
                available_mask: mask,
                owner
            }
            .validate()
            .is_err()
        );
    }
    let mut json = serde_json::to_value(&base).unwrap();
    json["oldMode"] = 2.into();
    assert!(serde_json::from_value::<profile::Target>(json).is_err());
}
