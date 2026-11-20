use super::*;
use sha2::{Digest, Sha256};
use std::cell::RefCell;

fn cycle_profile(
    f: &Fixture,
    memory: &Arc<Memory>,
    gpu: &crate::gpuctl::GpuController,
    power: &gpu_power::Controller,
) -> mode::cycle::Profile {
    let _gate = f.hal.begin_write().unwrap();
    mode::cycle::read(&context(f, memory, gpu, power)).unwrap()
}

#[test]
fn full_cycle_preview_checks_both_supplies_without_enabling_a_closed_node() {
    let f = core();
    let (memory, gpu, environment, power) = setup();
    close_node(&environment);
    gpu.set_power_suspended(true).unwrap();
    let profile = cycle_profile(&f, &memory, &gpu, &power);
    assert_eq!(profile.clocks.choices.len(), profile.modes.len() * 2);
    assert!(profile.clocks.driver.is_none());
    assert!(profile.clocks.deferred_reason.is_some());
    assert!(
        profile
            .clocks
            .choices
            .iter()
            .all(|choice| choice.candidate.projected.is_none())
    );
    assert!(profile.identity().is_ok());
    assert!(f.device.0.lock().unwrap().events.is_empty());
    assert!(memory.0.lock().unwrap().writes.is_empty());
    assert!(environment.io.0.lock().unwrap().events.is_empty());
    assert_eq!(environment.authorizations.load(Ordering::SeqCst), 0);
}

#[test]
fn full_cycle_authorization_survives_owned_changes() {
    let f = core().with_energy(1);
    let (memory, gpu, environment, power) = setup();
    environment.io.0.lock().unwrap().device.automatic_power_off = true;
    f.device.0.lock().unwrap().pages[1][16] = 3;
    let profile = cycle_profile(&f, &memory, &gpu, &power);
    let identity = profile.identity().unwrap();
    let driver = profile.driver_identity().unwrap().to_owned();
    for _ in 0..3 {
        let mode = mode::cycle::execute(
            context(&f, &memory, &gpu, &power),
            &identity,
            Some(&driver),
            &mut |actual| {
                assert_eq!(actual, driver);
                Ok(())
            },
            &mut || Ok(()),
        )
        .unwrap();
        let current = cycle_profile(&f, &memory, &gpu, &power);
        assert_eq!(current.identity().unwrap(), identity);
        assert_eq!(f.device.0.lock().unwrap().pages[1][16], mode);
    }
}

#[test]
fn deferred_driver_enrollment_is_checked_before_frequency_outputs_and_is_stable() {
    let f = core();
    let (memory, gpu, environment, power) = setup();
    close_node(&environment);
    gpu.set_power_suspended(true).unwrap();
    let profile = cycle_profile(&f, &memory, &gpu, &power);
    let identity = profile.identity().unwrap();
    let learned = RefCell::new(None::<String>);
    mode::cycle::execute(
        context(&f, &memory, &gpu, &power),
        &identity,
        None,
        &mut |driver| {
            assert!(memory.0.lock().unwrap().writes.is_empty());
            let mut learned = learned.borrow_mut();
            if let Some(previous) = learned.as_ref() {
                assert_eq!(previous, driver);
            }
            learned.get_or_insert_with(|| driver.to_owned());
            Ok(())
        },
        &mut || Ok(()),
    )
    .unwrap();
    let current = cycle_profile(&f, &memory, &gpu, &power);
    assert_eq!(current.identity().unwrap(), identity);
    assert_eq!(learned.borrow().as_deref(), current.driver_identity());
    assert_ne!(current.driver_identity(), None);
    assert!(current.check_driver(Some(&"f".repeat(64))).is_err());
}

#[test]
fn rejected_driver_enrollment_restores_pre_enabled_node_without_core_or_frequency_output() {
    let f = core();
    let (memory, gpu, environment, power) = setup();
    close_node(&environment);
    gpu.set_power_suspended(true).unwrap();
    let identity = cycle_profile(&f, &memory, &gpu, &power).identity().unwrap();
    let raw = RawState::read(memory.as_ref()).unwrap();
    let error = mode::cycle::execute(
        context(&f, &memory, &gpu, &power),
        &identity,
        None,
        &mut |_| anyhow::bail!("隔离原子授权写入失败"),
        &mut || Ok(()),
    )
    .unwrap_err();
    assert!(format!("{error:#}").contains("原子授权"));
    assert!(!environment.io.device().enabled);
    assert_eq!(RawState::read(memory.as_ref()).unwrap(), raw);
    assert!(f.device.0.lock().unwrap().events.is_empty());
    assert!(memory.0.lock().unwrap().writes.is_empty());
    assert!(f.hal.recovery().unwrap().oem_mode.is_none());
    assert_eq!(environment.authorizations.load(Ordering::SeqCst), 1);
}

#[test]
fn old_core_only_grant_and_changed_gpu_policy_cannot_gain_cycle_outputs() {
    let f = core();
    let (memory, gpu, environment, power) = setup();
    let profile = cycle_profile(&f, &memory, &gpu, &power);
    let old_targets: Vec<_> = profile
        .modes
        .iter()
        .map(|target| (target.mode, target.ac_mode, target.energy))
        .collect();
    let available: Vec<_> = profile.modes.iter().map(|target| target.mode).collect();
    let old = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&(available, old_targets)).unwrap())
    );
    assert_ne!(old, profile.identity().unwrap());
    assert!(
        mode::cycle::execute(
            context(&f, &memory, &gpu, &power),
            &old,
            None,
            &mut |_| Ok(()),
            &mut || Ok(())
        )
        .is_err()
    );
    let identity = profile.identity().unwrap();
    environment.io.0.lock().unwrap().device.automatic_power_off = true;
    assert!(
        mode::cycle::execute(
            context(&f, &memory, &gpu, &power),
            &identity,
            None,
            &mut |_| Ok(()),
            &mut || Ok(())
        )
        .is_err()
    );
    assert!(f.device.0.lock().unwrap().events.is_empty());
    assert!(memory.0.lock().unwrap().writes.is_empty());
    assert_eq!(environment.authorizations.load(Ordering::SeqCst), 0);
}

#[test]
fn cancelling_cycle_during_authorization_leaves_all_domains_and_first_baseline_untouched() {
    use std::sync::atomic::AtomicBool;
    let f = core();
    let (memory, gpu, environment, power) = setup();
    close_node(&environment);
    gpu.set_power_suspended(true).unwrap();
    let identity = cycle_profile(&f, &memory, &gpu, &power).identity().unwrap();
    let revoked = Arc::new(AtomicBool::new(false));
    let cancel = revoked.clone();
    environment.io.0.lock().unwrap().authorize_hook = Some(Box::new(move || {
        cancel.store(true, Ordering::SeqCst);
    }));
    let result = mode::cycle::execute(
        context(&f, &memory, &gpu, &power),
        &identity,
        None,
        &mut |_| Ok(()),
        &mut || {
            if revoked.load(Ordering::SeqCst) {
                anyhow::bail!("监听已停止");
            }
            Ok(())
        },
    );
    assert!(result.is_err());
    assert!(!environment.io.device().enabled);
    assert!(f.device.0.lock().unwrap().events.is_empty());
    assert!(memory.0.lock().unwrap().writes.is_empty());
    assert!(environment.io.0.lock().unwrap().events.is_empty());
    assert!(f.hal.recovery().unwrap().oem_mode.is_none());
    assert_eq!(environment.authorizations.load(Ordering::SeqCst), 1);
}
