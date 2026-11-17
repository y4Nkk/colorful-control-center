use super::*;
use crate::{
    gpuctl::turbo::mode::{self, RawState, Write},
    hal::nvapi::{clock::LockEntry, transaction::RawOffsets, turbo::DeviceId},
};

impl Memory {
    pub(super) fn apply_clocks(
        &self,
        attempt: &Attempt,
        checkpoint: &mut dyn FnMut(&Write, ClockOrigin, &str) -> Result<()>,
    ) -> Result<()> {
        let decision = attempt.clock_decision()?;
        let Some(target) = decision.target else {
            return Ok(());
        };
        let before = self
            .0
            .borrow()
            .clocks
            .clone()
            .ok_or_else(|| anyhow::anyhow!("隔离GPU不可读"))?;
        let write = Write {
            after: before.projected(&target)?,
            before,
        };
        let origin = if attempt.before.gpu_clocks.raw().is_some() {
            ClockOrigin::BeforeMode
        } else if self.gpu_power()?.is_some_and(|gpu| gpu.target.enabled) {
            ClockOrigin::AfterEnable
        } else {
            ClockOrigin::PoweredOff
        };
        checkpoint(&write, origin, &self.0.borrow().driver_identity)?;
        let mut data = self.0.borrow_mut();
        data.clocks = Some(write.after);
        data.clock_writes += 1;
        Ok(())
    }
}

pub(super) fn raw() -> RawState {
    RawState {
        devices: vec![DeviceId {
            pci_device: 0x28a0_10de,
            subsystem: 1,
            revision: 1,
            extended: 0x28a0,
            bus: 1,
            slot: 0,
        }],
        locks: [
            LockEntry {
                domain: 0,
                mode: 3,
                value: 999_999,
                unknown1: 42,
                ..Default::default()
            },
            LockEntry {
                domain: 1,
                ..Default::default()
            },
        ],
        offsets: vec![RawOffsets {
            core_khz: Some(12_345),
            mem_khz: Some(-54_321),
        }],
    }
}
pub(super) fn with_clocks(io: &Memory, enabled: bool) {
    gpu_tests::with_gpu(io, enabled, true);
    let mut policy = mode::test_policy();
    policy.ready = true;
    policy.gpu_count = 1;
    policy.gpu_oc = Some(true);
    policy.capabilities.as_mut().unwrap().limit_clock = true;
    policy.limits.performance_mhz = 1700;
    policy.saved[0] = ccc_core::gpu_turbo::Offsets {
        core_mhz: 100,
        memory_mhz: 200,
    };
    let mut data = io.0.borrow_mut();
    data.review.on_ac = Some(true);
    let mut review = mode::review(
        policy,
        [Some(2), Some(1), Some(2), Some(1), Some(2), Some(1)],
    );
    if enabled {
        review.driver = Some(mode::DriverReview {
            identity: data.driver_identity.clone(),
            current: raw(),
            devices: vec![],
        });
    }
    data.review.clocks = Some(review);
    data.clocks = Some(raw());
}

#[test]
fn combined_transaction_persists_raw_frequency_baseline_and_restores_exact_values() {
    let (_dir, store, io) = setup();
    with_clocks(&io, true);
    enable(&io, &store);
    transaction::execute(&io, &store, Trigger::SupplyChange, &mut || Ok(())).unwrap();
    assert_eq!(io.0.borrow().clock_writes, 1);
    let record = store.load().unwrap().unwrap();
    assert!(record.track_clocks && record.attempt.is_none());
    let initial = record
        .initial
        .as_ref()
        .unwrap()
        .gpu_clocks
        .as_ref()
        .unwrap();
    assert_eq!(initial.origin, ClockOrigin::BeforeMode);
    assert_eq!(initial.state, raw());
    let view = transaction::read(&io, &store).unwrap();
    assert_eq!(
        view.recovery
            .as_ref()
            .unwrap()
            .initial_gpu_clocks
            .as_ref()
            .unwrap()
            .state,
        raw()
    );
    transaction::restore(&io, &store, &view.revision, &mut || Ok(())).unwrap();
    assert_eq!(io.0.borrow().clocks.as_ref().unwrap(), &raw());
    assert!(store.load().unwrap().is_none());
}

#[test]
fn first_read_after_enabling_is_labeled_and_restored_before_returning_to_off() {
    let (_dir, store, io) = setup();
    with_clocks(&io, false);
    enable(&io, &store);
    transaction::execute(&io, &store, Trigger::SupplyChange, &mut || Ok(())).unwrap();
    let record = store.load().unwrap().unwrap();
    let initial = record.initial.as_ref().unwrap();
    assert!(!initial.gpu_power.as_ref().unwrap().target.enabled);
    assert_eq!(
        initial.gpu_clocks.as_ref().unwrap().origin,
        ClockOrigin::AfterEnable
    );
    assert!(matches!(
        initial.expected().gpu_clocks,
        ClockObservation::PoweredOff
    ));
    let view = transaction::read(&io, &store).unwrap();
    transaction::restore(&io, &store, &view.revision, &mut || Ok(())).unwrap();
    assert!(!io.gpu_power().unwrap().unwrap().target.enabled);
    assert_eq!(io.0.borrow().clocks.as_ref().unwrap(), &raw());
    assert!(store.load().unwrap().is_none());
}

#[test]
fn external_frequency_changes_pause_automation_and_invalidate_the_old_preview() {
    let (_dir, store, io) = setup();
    with_clocks(&io, true);
    enable(&io, &store);
    transaction::execute(&io, &store, Trigger::SupplyChange, &mut || Ok(())).unwrap();
    let old = transaction::read(&io, &store).unwrap();
    io.0.borrow_mut().clocks.as_mut().unwrap().offsets[0].core_khz = Some(222_222);
    let before = io.0.borrow().clock_writes;
    assert!(transaction::execute(&io, &store, Trigger::SupplyChange, &mut || Ok(())).is_err());
    assert!(transaction::restore(&io, &store, &old.revision, &mut || Ok(())).is_err());
    assert_eq!(io.0.borrow().clock_writes, before);
    let current = transaction::read(&io, &store).unwrap();
    assert!(current.enabled && current.can_restore && current.error.is_some());
    transaction::restore(&io, &store, &current.revision, &mut || Ok(())).unwrap();
    assert_eq!(io.0.borrow().clocks.as_ref().unwrap(), &raw());
}

#[test]
fn replacing_a_gpu_blocks_every_recovery_domain_before_output() {
    let (_dir, store, io) = setup();
    with_clocks(&io, true);
    enable(&io, &store);
    transaction::execute(&io, &store, Trigger::SupplyChange, &mut || Ok(())).unwrap();
    io.0.borrow_mut().clocks.as_mut().unwrap().devices[0].bus = 5;
    let before = io.0.borrow().writes;
    let view = transaction::read(&io, &store).unwrap();
    assert!(!view.can_restore && view.recovery_error.as_ref().unwrap().contains("GPU频率"));
    assert!(transaction::restore(&io, &store, &view.revision, &mut || Ok(())).is_err());
    assert_eq!(io.0.borrow().writes, before);
    assert!(store.load().unwrap().is_some());
}

#[test]
fn external_record_replacement_cannot_authorize_a_frequency_write() {
    let (_dir, store, io) = setup();
    with_clocks(&io, true);
    enable(&io, &store);
    let mut checks = 0;
    assert!(
        transaction::execute(&io, &store, Trigger::SupplyChange, &mut || {
            checks += 1;
            if checks == 2 {
                let old = store.load()?.unwrap();
                let mut other = old.clone();
                other.identity = "c".repeat(64);
                store.save(Some(&old), &other)?;
            }
            Ok(())
        })
        .is_err()
    );
    assert_eq!(io.0.borrow().clock_writes, 0);
    assert!(store.load().unwrap().unwrap().attempt.is_some());
}

#[test]
fn later_failure_keeps_the_frequency_checkpoint_for_explicit_recovery() {
    let (_dir, store, io) = setup();
    with_clocks(&io, true);
    enable(&io, &store);
    io.0.borrow_mut().fail_after = true;
    assert!(transaction::execute(&io, &store, Trigger::SupplyChange, &mut || Ok(())).is_err());
    let record = store.load().unwrap().unwrap();
    let attempt = record.attempt.unwrap();
    assert_eq!(attempt.clock_write.as_ref().unwrap().before, raw());
    let view = transaction::read(&io, &store).unwrap();
    assert!(view.can_restore && view.enabled && view.error.is_some());
    transaction::restore(&io, &store, &view.revision, &mut || Ok(())).unwrap();
    assert_eq!(io.0.borrow().clocks.as_ref().unwrap(), &raw());
}

#[test]
fn quiet_mode_records_off_control_before_its_later_gpu_enable() {
    let (_dir, store, io) = setup();
    with_clocks(&io, false);
    {
        let mut data = io.0.borrow_mut();
        let quiet = target(0);
        data.review.ac_mode = 0;
        data.review.ac = Some(quiet.clone());
        data.review.targets.push(quiet);
        data.review.gpu = super::super::gpu::review(
            true,
            &[data.review.gpu.current.clone().unwrap()],
            true,
            [Some(0), Some(1), Some(2), Some(1), Some(2), Some(1)],
        )
        .unwrap();
        data.review.clocks = Some(mode::review(
            data.review.clocks.as_ref().unwrap().policy.clone(),
            [Some(0), Some(1), Some(2), Some(1), Some(2), Some(1)],
        ));
    }
    enable(&io, &store);
    transaction::execute(&io, &store, Trigger::SupplyChange, &mut || Ok(())).unwrap();
    let record = store.load().unwrap().unwrap();
    let baseline = record
        .initial
        .as_ref()
        .unwrap()
        .gpu_clocks
        .as_ref()
        .unwrap();
    assert_eq!(baseline.origin, ClockOrigin::PoweredOff);
    assert!(io.gpu_power().unwrap().unwrap().target.enabled);
    let view = transaction::read(&io, &store).unwrap();
    transaction::restore(&io, &store, &view.revision, &mut || Ok(())).unwrap();
    assert!(!io.gpu_power().unwrap().unwrap().target.enabled);
    assert_eq!(io.0.borrow().clocks.as_ref().unwrap(), &raw());
}
