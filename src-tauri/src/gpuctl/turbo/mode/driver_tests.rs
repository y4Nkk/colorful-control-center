use super::*;
use crate::gpuctl::turbo::mode::{self, Review};

fn view(memory: &Memory) -> Review {
    mode::review(
        policy(memory),
        [Some(2), Some(2), Some(3), Some(2), Some(2), Some(1)],
    )
}
fn inspect(memory: &Memory, view: &mut Review) -> Result<()> {
    view.inspect_driver(
        &NoBase(memory),
        memory,
        [Some(2), Some(2), Some(3), Some(2), Some(2), Some(1)],
        Some(true),
    )
}
#[test]
fn preview_validates_future_supply_targets_without_changing_actual_source_or_reading_base_clock() {
    let memory = fixture();
    memory.0.lock().unwrap().source.limits.thermal_mhz = 0;
    let mut view = view(&memory);
    inspect(&memory, &mut view).unwrap();
    assert!(view.available());
    let current = &view.driver.as_ref().unwrap().current;
    assert_eq!(current.offsets[0].core_khz, Some(10_000));
    assert_eq!(
        view.ac.projected.as_ref().unwrap().offsets[0].core_khz,
        Some(100_000)
    );
    assert_eq!(
        view.battery.projected.as_ref().unwrap().offsets[0].core_khz,
        Some(0)
    );
    assert_eq!(
        view.battery.projected.as_ref().unwrap().locks,
        current.locks
    );
    assert_eq!(
        view.startup_ac.projected.as_ref().unwrap().locks[0].value,
        1_400_000
    );
    assert_eq!(
        view.startup_battery.projected.as_ref().unwrap().offsets[0].core_khz,
        Some(0)
    );
    assert!(view.resume_ac.projected.is_none() && view.resume_battery.projected.is_none());
    let data = memory.0.lock().unwrap();
    assert_eq!((data.source.mode, data.source.on_ac), (2, Some(true)));
    assert!(data.writes.is_empty());
}
#[test]
fn individual_range_failures_remain_visible_and_do_not_hide_other_candidates_or_current_readings() {
    let memory = fixture();
    memory.0.lock().unwrap().source.saved[0].core_mhz = 400;
    let mut view = view(&memory);
    inspect(&memory, &mut view).unwrap();
    assert!(!view.available());
    assert!(view.driver.is_some() && view.driver_error.is_none());
    assert!(view.ac.unavailable_reason.is_some() && view.ac.projected.is_none());
    assert!(view.battery.unavailable_reason.is_none() && view.battery.projected.is_some());
    assert!(view.resume_ac.unavailable_reason.is_none() && view.resume_ac.projected.is_none());
    assert!(
        view.resume_battery.unavailable_reason.is_none() && view.resume_battery.projected.is_none()
    );
    assert!(memory.0.lock().unwrap().writes.is_empty());
}
#[test]
fn driver_identity_binds_complete_device_and_capabilities_while_raw_values_are_separate() {
    let memory = fixture();
    let mut first = view(&memory);
    inspect(&memory, &mut first).unwrap();
    let before = first.driver.unwrap();
    memory.0.lock().unwrap().offsets[0].core.delta_khz += 1;
    let mut changed = view(&memory);
    inspect(&memory, &mut changed).unwrap();
    let actual = changed.driver.unwrap();
    assert_eq!(before.identity, actual.identity);
    assert_ne!(before.current, actual.current);
    memory.0.lock().unwrap().offsets[0].core.max_khz -= 1;
    let mut changed = view(&memory);
    inspect(&memory, &mut changed).unwrap();
    assert_ne!(actual.identity, changed.driver.unwrap().identity);
    memory.0.lock().unwrap().devices[0].slot += 1;
    let mut changed = view(&memory);
    assert!(inspect(&memory, &mut changed).is_err());
    assert!(changed.driver.is_none());
}
#[test]
fn mismatched_cached_identity_or_collection_cannot_produce_a_driver_preview() {
    let memory = fixture();
    for count in [1, 3] {
        let mut view = view(&memory);
        view.policy.gpu_count = count;
        assert!(inspect(&memory, &mut view).is_err());
        assert!(view.driver.is_none());
    }
    let mut view = view(&memory);
    view.policy.cached_device_id ^= 1;
    assert!(inspect(&memory, &mut view).is_err());
    assert!(memory.0.lock().unwrap().writes.is_empty());
}
#[test]
fn powered_off_preview_is_explicitly_deferred_and_does_not_reconnect_the_gpu() {
    let memory = std::sync::Arc::new(fixture());
    let mut view = view(&memory);
    let gpu = isolated_controller(memory.clone());
    gpu.set_power_suspended(true).unwrap();
    gpu.inspect_mode_gpu_in_control(&mut view, memory.as_ref(), [Some(2); 6], Some(true), true);
    assert!(view.deferred_reason.is_some() && view.driver.is_none() && view.driver_error.is_none());
    assert_eq!(memory.0.lock().unwrap().reads, 0);
    assert!(gpu.state.lock().unwrap().power_suspended);
    assert!(!gpu.read_profile().available);
}
#[test]
fn skipped_policy_never_initializes_an_unneeded_gpu_provider() {
    let memory = std::sync::Arc::new(fixture());
    let mut policy = policy(&memory);
    policy.ready = false;
    let mut view = mode::review(policy, [Some(2); 6]);
    isolated_controller(memory.clone()).inspect_mode_gpu_in_control(
        &mut view,
        memory.as_ref(),
        [Some(2); 6],
        Some(true),
        false,
    );
    assert!(view.available() && view.driver.is_none() && view.deferred_reason.is_none());
    assert_eq!(memory.0.lock().unwrap().reads, 0);
}
#[test]
fn a_mismatched_or_unknown_actual_supply_does_not_become_a_future_synthetic_reading() {
    let memory = fixture();
    memory.0.lock().unwrap().source.on_ac = Some(false);
    let mut view = view(&memory);
    assert!(inspect(&memory, &mut view).is_err());
    assert!(view.driver.is_none());
    memory.0.lock().unwrap().source.on_ac = None;
    assert!(inspect(&memory, &mut view).is_err());
    assert!(view.driver.is_none());
}

#[test]
fn a_resume_only_mask_does_not_open_a_driver_for_other_future_candidates() {
    let memory = std::sync::Arc::new(fixture());
    let mut view = view(&memory);
    assert!(view.ac.decision.as_ref().unwrap().target.is_some());
    let gpu = isolated_controller(memory.clone());
    gpu.set_power_suspended(true).unwrap();
    let mut modes = [None; 6];
    modes[4] = Some(2);
    gpu.inspect_mode_gpu_in_control(&mut view, memory.as_ref(), modes, Some(true), false);
    assert!(view.driver.is_none() && view.driver_error.is_none());
    assert_eq!(memory.0.lock().unwrap().reads, 0);
    let state = gpu.state.lock().unwrap();
    assert!(state.session.is_none() && state.turbo_io.is_none());
}
