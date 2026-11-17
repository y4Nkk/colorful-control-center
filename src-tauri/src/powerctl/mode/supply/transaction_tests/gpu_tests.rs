use super::*;
use crate::gpu_power::mode as gpu;
use crate::gpu_power::{Device, Target as GpuTarget};

pub(super) fn with_gpu(io: &Memory, enabled: bool, automatic: bool) {
    let mut data = io.0.borrow_mut();
    data.review.gpu = crate::powerctl::mode::supply::gpu::review(
        true,
        &[Device {
            id: "ACPI\\NVDA0820\\SUPPLY_TEST".into(),
            name: "Isolated GPU".into(),
            enabled,
            automatic_power_off: automatic,
            problem: if enabled { 0 } else { 22 },
            drives_display: false,
            can_disable: true,
        }],
        true,
        [Some(2), Some(1), Some(2), Some(1), Some(2), Some(1)],
    )
    .unwrap();
}

#[test]
fn current_ac_does_not_hide_an_unavailable_future_battery_resume_shutdown() {
    let (_dir, store, io) = setup();
    with_gpu(&io, true, true);
    {
        let mut data = io.0.borrow_mut();
        data.review.on_ac = Some(true);
        let mut device = data.review.gpu.current.clone().unwrap();
        device.drives_display = true;
        device.can_disable = false;
        data.review.gpu = crate::powerctl::mode::supply::gpu::review(
            true,
            &[device],
            true,
            [Some(2), Some(1), Some(2), Some(1), Some(2), Some(1)],
        )
        .unwrap();
        assert!(data.review.gpu.ac.unavailable_reason.is_none());
        assert!(data.review.gpu.resume_ac.unavailable_reason.is_none());
        assert!(data.review.gpu.resume_battery.unavailable_reason.is_some());
    }
    let view = transaction::read(&io, &store).unwrap();
    assert!(!view.can_enable);
    assert!(transaction::enable(&io, &store, &view.revision, &mut || Ok(())).is_err());
    assert!(store.load().unwrap().is_none());
    assert_eq!(io.0.borrow().writes, 0);
}

#[test]
fn enabling_gpu_preserves_auto_off_preference_and_restores_the_first_combination() {
    for automatic in [true, false] {
        let (_dir, store, io) = setup();
        io.0.borrow_mut().review.on_ac = Some(true);
        with_gpu(&io, false, automatic);
        let original_mode = io.0.borrow().snapshot.clone();
        let original_plans = io.0.borrow().plans.clone();
        enable(&io, &store);
        assert_eq!(io.0.borrow().writes, 0);
        transaction::execute(&io, &store, Trigger::SupplyChange, &mut || Ok(())).unwrap();
        let applied = io.gpu_power().unwrap().unwrap();
        assert_eq!(
            applied.target,
            GpuTarget {
                enabled: true,
                automatic_power_off: automatic
            }
        );
        let view = transaction::read(&io, &store).unwrap();
        let recovery = view.recovery.as_ref().unwrap();
        assert_eq!(recovery.current_gpu_power.as_ref().unwrap(), &applied);
        assert_eq!(
            recovery.initial_gpu_power.as_ref().unwrap().target,
            GpuTarget {
                enabled: false,
                automatic_power_off: automatic
            }
        );
        transaction::restore(&io, &store, &view.revision, &mut || Ok(())).unwrap();
        assert!(!io.gpu_power().unwrap().unwrap().target.enabled);
        assert_eq!(
            io.gpu_power().unwrap().unwrap().target.automatic_power_off,
            automatic
        );
        assert!(io.0.borrow().snapshot == original_mode);
        assert!(io.0.borrow().plans == original_plans);
        assert!(store.load().unwrap().is_none());
    }
}

#[test]
fn external_gpu_change_stops_automation_and_invalidates_the_previous_recovery_preview() {
    let (_dir, store, io) = setup();
    io.0.borrow_mut().review.on_ac = Some(true);
    with_gpu(&io, false, true);
    enable(&io, &store);
    transaction::execute(&io, &store, Trigger::SupplyChange, &mut || Ok(())).unwrap();
    let old = transaction::read(&io, &store).unwrap();
    with_gpu(&io, false, true);
    let before = io.0.borrow().writes;
    assert!(transaction::execute(&io, &store, Trigger::SupplyChange, &mut || Ok(())).is_err());
    assert!(transaction::restore(&io, &store, &old.revision, &mut || Ok(())).is_err());
    assert_eq!(io.0.borrow().writes, before);
    let current = transaction::read(&io, &store).unwrap();
    assert!(current.enabled && current.can_restore && current.error.is_some());
    assert!(current.error.as_ref().unwrap().contains("独显"));
    transaction::restore(&io, &store, &current.revision, &mut || Ok(())).unwrap();
    assert!(store.load().unwrap().is_none());
}

#[test]
fn display_in_use_prevents_any_domain_restore_until_a_new_preview_is_confirmed() {
    let (_dir, store, io) = setup();
    io.0.borrow_mut().review.on_ac = Some(true);
    with_gpu(&io, false, true);
    enable(&io, &store);
    transaction::execute(&io, &store, Trigger::SupplyChange, &mut || Ok(())).unwrap();
    let before = io.0.borrow().writes;
    let mode = io.0.borrow().snapshot.clone();
    let plans = io.0.borrow().plans.clone();
    {
        let mut data = io.0.borrow_mut();
        let device = data.review.gpu.current.as_mut().unwrap();
        device.drives_display = true;
        device.can_disable = false;
    }
    let blocked = transaction::read(&io, &store).unwrap();
    assert!(!blocked.can_restore && blocked.recovery_error.is_some());
    assert!(transaction::restore(&io, &store, &blocked.revision, &mut || Ok(())).is_err());
    assert_eq!(io.0.borrow().writes, before);
    assert!(io.0.borrow().snapshot == mode && io.0.borrow().plans == plans);
    assert!(store.load().unwrap().is_some());
    {
        let mut data = io.0.borrow_mut();
        let device = data.review.gpu.current.as_mut().unwrap();
        device.drives_display = false;
        device.can_disable = true;
    }
    let ready = transaction::read(&io, &store).unwrap();
    assert!(ready.can_restore && ready.revision != blocked.revision);
    assert!(transaction::restore(&io, &store, &blocked.revision, &mut || Ok(())).is_err());
    transaction::restore(&io, &store, &ready.revision, &mut || Ok(())).unwrap();
    assert!(store.load().unwrap().is_none());
}

#[test]
fn partial_gpu_mode_failure_keeps_its_journal_and_recovers_all_initial_domains() {
    let (_dir, store, io) = setup();
    io.0.borrow_mut().review.on_ac = Some(true);
    with_gpu(&io, false, true);
    let mode = io.0.borrow().snapshot.clone();
    let plans = io.0.borrow().plans.clone();
    let energy = io.0.borrow().snapshot.energy().unwrap();
    let gpu = io.gpu_power().unwrap();
    enable(&io, &store);
    io.0.borrow_mut().fail_after = true;
    assert!(transaction::execute(&io, &store, Trigger::SupplyChange, &mut || Ok(())).is_err());
    let record = store.load().unwrap().unwrap();
    let attempt = record.attempt.unwrap();
    assert_eq!(attempt.gpu_action.unwrap().stage, gpu::Stage::BeforeMode);
    assert_eq!(attempt.before.gpu_power, gpu);
    let view = transaction::read(&io, &store).unwrap();
    assert!(view.enabled && view.can_restore && view.error.is_some());
    transaction::restore(&io, &store, &view.revision, &mut || Ok(())).unwrap();
    assert!(io.0.borrow().snapshot == mode && io.0.borrow().plans == plans);
    assert_eq!(io.0.borrow().snapshot.energy().unwrap(), energy);
    assert_eq!(io.gpu_power().unwrap(), gpu);
    assert!(store.load().unwrap().is_none());
}
