use super::{
    Review, Scope, Target,
    plans::{self, PlanValue},
    record::{Attempt, ClockRead, FileStore, Initial, Snapshot},
    transaction::{self, Io},
};
use crate::gpuctl::turbo::mode::{ClockObservation, ClockOrigin};
use crate::powerctl::mode::workflow::StoredModeSnapshot;
use anyhow::{Result, bail};
use ccc_core::oem_supply_mode::Trigger;
mod clock_tests;
mod gpu_tests;
mod memory;
mod resume_tests;
pub(super) use memory::{Memory, setup};
use memory::{enable, snapshot, target};

#[test]
fn automatic_journal_rejects_ac_preference_targets_and_snapshots() {
    let (_dir, store, io) = setup();
    enable(&io, &store);
    transaction::execute(&io, &store, Trigger::SupplyChange, &mut || Ok(())).unwrap();
    let original = store.load().unwrap().unwrap();
    let mut value = serde_json::to_value(&original).unwrap();
    value["initial"]["mode"]["core"]["acMode"] = 3.into();
    let changed = serde_json::from_value(value).unwrap();
    assert!(store.save(Some(&original), &changed).is_err());
    let mut changed = original.clone();
    changed.capture_target.as_mut().unwrap().ac_mode = Some(1);
    assert!(store.save(Some(&original), &changed).is_err());
    assert!(store.load().unwrap().unwrap() == original);
}

#[test]
fn first_write_refuses_a_mode_change_between_review_and_the_complete_snapshot() {
    let (_dir, store, io) = setup();
    enable(&io, &store);
    let energy = io.0.borrow().snapshot.energy().unwrap();
    io.0.borrow_mut().snapshot = snapshot(3, 0, energy);
    assert!(transaction::execute(&io, &store, Trigger::Startup, &mut || Ok(())).is_err());
    assert!(store.load().unwrap().unwrap().initial.is_none());
    assert_eq!(io.0.borrow().writes, 0);
}
#[test]
fn explicit_authorization_precedes_mode_write_and_restore_uses_first_snapshot() {
    let (_dir, store, io) = setup();
    enable(&io, &store);
    assert_eq!(io.0.borrow().writes, 0);
    let applied = transaction::execute(&io, &store, Trigger::SupplyChange, &mut || Ok(()))
        .unwrap()
        .unwrap();
    assert_eq!(
        (applied.previous_mode, applied.mode, applied.on_ac),
        (2, 1, false)
    );
    let view = transaction::read(&io, &store).unwrap();
    assert!(view.enabled && view.can_restore && view.recovery.is_some());
    transaction::restore(&io, &store, &view.revision, &mut || Ok(())).unwrap();
    assert_eq!(io.0.borrow().review.current_mode, 2);
    assert_eq!(io.0.borrow().writes, 2);
    assert!(store.load().unwrap().is_none());
}
#[test]
fn external_change_rejects_auto_and_stale_recovery_requires_new_preview() {
    let (_dir, store, io) = setup();
    enable(&io, &store);
    transaction::execute(&io, &store, Trigger::SupplyChange, &mut || Ok(())).unwrap();
    let old = transaction::read(&io, &store).unwrap();
    let energy = io.0.borrow().snapshot.energy().unwrap();
    io.0.borrow_mut().snapshot = snapshot(1, 30, energy);
    assert!(transaction::execute(&io, &store, Trigger::SupplyChange, &mut || Ok(())).is_err());
    assert!(transaction::restore(&io, &store, &old.revision, &mut || Ok(())).is_err());
    assert_eq!(io.0.borrow().writes, 1);
    let current = transaction::read(&io, &store).unwrap();
    assert!(
        current.recovery.as_ref().unwrap().initial_fan_offset
            != current.recovery.as_ref().unwrap().current_fan_offset
    );
    transaction::restore(&io, &store, &current.revision, &mut || Ok(())).unwrap();
    assert!(store.load().unwrap().is_none());
}
#[test]
fn partial_failure_keeps_journal_and_restore_rechecks_the_confirmed_state() {
    let (_dir, store, io) = setup();
    enable(&io, &store);
    io.0.borrow_mut().fail_after = true;
    assert!(transaction::execute(&io, &store, Trigger::SupplyChange, &mut || Ok(())).is_err());
    assert!(store.load().unwrap().unwrap().attempt.is_some());
    let view = transaction::read(&io, &store).unwrap();
    assert!(view.error.is_some() && view.can_restore);
    io.0.borrow_mut().mutate_before_restore = true;
    assert!(transaction::restore(&io, &store, &view.revision, &mut || Ok(())).is_err());
    assert!(store.load().unwrap().is_some());
    io.0.borrow_mut().mutate_before_restore = false;
    let view = transaction::read(&io, &store).unwrap();
    transaction::restore(&io, &store, &view.revision, &mut || Ok(())).unwrap();
    assert_eq!(io.0.borrow().review.current_mode, 2);
}
#[test]
fn authorization_and_device_changes_prevent_unreviewed_output() {
    let (_dir, store, io) = setup();
    let view = transaction::read(&io, &store).unwrap();
    assert!(transaction::enable(&io, &store, &view.revision, &mut || bail!("撤销")).is_err());
    assert!(store.load().unwrap().is_none());
    enable(&io, &store);
    io.0.borrow_mut().review.identity = "c".repeat(64);
    assert!(transaction::execute(&io, &store, Trigger::SupplyChange, &mut || Ok(())).is_err());
    assert_eq!(io.0.borrow().writes, 0);
    io.0.borrow_mut().review.device_identity = "d".repeat(64);
    assert!(!transaction::read(&io, &store).unwrap().can_restore);
}

#[test]
fn multiple_plans_are_reviewed_restored_and_external_changes_require_a_new_preview() {
    let (_dir, store, io) = setup();
    let original = io.0.borrow().plans.clone();
    enable(&io, &store);
    transaction::execute(&io, &store, Trigger::SupplyChange, &mut || Ok(())).unwrap();
    io.0.borrow_mut().review.on_ac = Some(true);
    transaction::execute(&io, &store, Trigger::SupplyChange, &mut || Ok(())).unwrap();
    let before = transaction::read(&io, &store).unwrap();
    assert_eq!(before.recovery.as_ref().unwrap().plan_changes.len(), 2);
    io.0.borrow_mut().plans[0].aspm = [0, 0];
    assert!(transaction::restore(&io, &store, &before.revision, &mut || Ok(())).is_err());
    let current = transaction::read(&io, &store).unwrap();
    assert!(current.enabled && current.can_restore && current.error.is_some());
    transaction::restore(&io, &store, &current.revision, &mut || Ok(())).unwrap();
    assert_eq!(io.0.borrow().plans, original);
}

#[test]
fn external_energy_change_stops_automation_and_is_visible_before_recovery() {
    let (_dir, store, io) = setup();
    enable(&io, &store);
    transaction::execute(&io, &store, Trigger::SupplyChange, &mut || Ok(())).unwrap();
    let old = transaction::read(&io, &store).unwrap();
    io.set_energy(crate::powerctl::mode::energy::State {
        hardware_p_state: false,
        energy_star: Some(2),
    });
    assert!(transaction::execute(&io, &store, Trigger::SupplyChange, &mut || Ok(())).is_err());
    assert!(transaction::restore(&io, &store, &old.revision, &mut || Ok(())).is_err());
    let updated = transaction::read(&io, &store).unwrap();
    let preview = updated.recovery.as_ref().unwrap();
    assert_eq!(preview.current_energy.unwrap().energy_star, Some(2));
    assert_eq!(preview.initial_energy.unwrap().energy_star, Some(0));
    transaction::restore(&io, &store, &updated.revision, &mut || Ok(())).unwrap();
    assert_eq!(
        io.0.borrow().snapshot.energy().unwrap().energy_star,
        Some(0)
    );
}

#[test]
fn changed_energy_capability_cannot_partially_restore_other_domains() {
    let (_dir, store, io) = setup();
    enable(&io, &store);
    transaction::execute(&io, &store, Trigger::SupplyChange, &mut || Ok(())).unwrap();
    let before = io.0.borrow().writes;
    io.set_energy(crate::powerctl::mode::energy::State {
        hardware_p_state: true,
        energy_star: None,
    });
    let view = transaction::read(&io, &store).unwrap();
    assert!(view.enabled && !view.can_restore && view.error.is_some());
    assert!(transaction::restore(&io, &store, &view.revision, &mut || Ok(())).is_err());
    assert_eq!(io.0.borrow().writes, before);
    assert!(store.load().unwrap().is_some());
}
