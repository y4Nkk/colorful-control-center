use super::*;
#[path = "combined_tests.rs"]
mod combined_tests;
#[path = "selection_tests.rs"]
mod selection_tests;
use crate::{
    gpu_power::{self, mode::State as NodeState, test_fixture::EnvironmentFixture},
    gpuctl::turbo::mode::{ClockObservation, ClockOrigin},
    powerctl::mode::links::Context,
};
use std::sync::{Arc, atomic::Ordering};

struct SourceFixture(Arc<Memory>);
impl SourceIo for SourceFixture {
    fn read(&self) -> Result<Source> {
        self.0.read()
    }
}
impl crate::powerctl::mode::links::Source for SourceFixture {
    fn on_ac(&self) -> Result<Option<bool>> {
        Ok(self.0.0.lock().unwrap().source.on_ac)
    }
    fn policy(&self) -> Result<Policy> {
        Ok(policy(&self.0))
    }
}
fn setup() -> (
    Arc<Memory>,
    crate::gpuctl::GpuController,
    Arc<EnvironmentFixture>,
    gpu_power::Controller,
) {
    let memory = Arc::new(fixture());
    let gpu = isolated_controller(memory.clone());
    let environment = EnvironmentFixture::new();
    let power = gpu_power::Controller::with_environment(environment.clone());
    (memory, gpu, environment, power)
}
fn close_node(environment: &EnvironmentFixture) {
    let mut data = environment.io.0.lock().unwrap();
    data.device.enabled = false;
    data.device.problem = 22;
    data.firmware = false;
}

#[test]
fn mode_context_reads_without_waiting_for_the_already_held_node_scope() {
    let (_memory, gpu, environment, power) = setup();
    let hal = gpu_power::test_fixture::hal();
    let _scope = power.scope().unwrap();
    let context = Context::native(&hal, &gpu, &power);
    assert_eq!(context.devices().unwrap(), vec![environment.io.device()]);
    assert_eq!(environment.authorizations.load(Ordering::SeqCst), 0);
    assert!(environment.io.0.lock().unwrap().events.is_empty());
}

#[test]
fn linked_pre_enable_captures_real_clock_origin_and_restores_clocks_before_closing_node() {
    let (memory, gpu, environment, power) = setup();
    close_node(&environment);
    gpu.set_power_suspended(true).unwrap();
    let hal = gpu_power::test_fixture::hal();
    let policy = policy(&memory);
    let raw = RawState::read(memory.as_ref()).unwrap();
    let session = Context::with_source(&hal, &gpu, &power, Box::new(SourceFixture(memory.clone())))
        .session()
        .unwrap();
    let device = environment.io.device();
    let before = NodeState::from_device(&device).unwrap();
    session
        .freeze_node(&device, gpu_power::Target::requested(true))
        .unwrap();
    assert_eq!(session.read_clocks().unwrap(), None);
    session.authorize().unwrap();
    assert!(environment.io.0.lock().unwrap().events.is_empty());
    session
        .perform(&before, gpu_power::Target::requested(true), &mut || Ok(()))
        .unwrap();
    let mut origin = None;
    session
        .prepare_clocks(
            &policy,
            3,
            true,
            false,
            &ClockObservation::PoweredOff,
            Some(ClockOrigin::AfterEnable),
            &mut |_, actual, _| {
                origin = Some(actual);
                Ok(())
            },
            &mut || Ok(()),
        )
        .unwrap();
    assert_eq!(origin, Some(ClockOrigin::AfterEnable));
    assert!(memory.0.lock().unwrap().writes.is_empty());
    switch_mode(&memory);
    session
        .apply_clocks(
            &mut || Ok(policy.clone()),
            &mut |_, _, _| Ok(()),
            &mut || Ok(()),
        )
        .unwrap();
    assert_ne!(RawState::read(memory.as_ref()).unwrap(), raw);
    session.recover_effects(false).unwrap();
    assert_eq!(RawState::read(memory.as_ref()).unwrap(), raw);
    session.rollback_nodes().unwrap();
    assert!(!environment.io.device().enabled);
    session.finish().unwrap();
    assert_eq!(environment.authorizations.load(Ordering::SeqCst), 1);
}

#[test]
fn linked_quiet_mode_tunes_while_off_and_restores_after_mode_node_before_frequency() {
    let (memory, gpu, environment, power) = setup();
    close_node(&environment);
    gpu.set_power_suspended(true).unwrap();
    let hal = gpu_power::test_fixture::hal();
    let policy = policy(&memory);
    let raw = RawState::read(memory.as_ref()).unwrap();
    let session = Context::with_source(&hal, &gpu, &power, Box::new(SourceFixture(memory.clone())))
        .session()
        .unwrap();
    let device = environment.io.device();
    let before = NodeState::from_device(&device).unwrap();
    session
        .freeze_node(&device, gpu_power::Target::requested(true))
        .unwrap();
    session.authorize().unwrap();
    session
        .prepare_clocks(
            &policy,
            0,
            true,
            false,
            &ClockObservation::PoweredOff,
            Some(ClockOrigin::PoweredOff),
            &mut |_, origin, _| {
                assert_eq!(origin, ClockOrigin::PoweredOff);
                Ok(())
            },
            &mut || Ok(()),
        )
        .unwrap();
    assert!(!environment.io.device().enabled);
    assert!(!gpu.read_profile().available);
    memory.0.lock().unwrap().source.mode = 0;
    session
        .apply_clocks(
            &mut || Ok(policy.clone()),
            &mut |_, _, _| Ok(()),
            &mut || Ok(()),
        )
        .unwrap();
    session
        .perform(&before, gpu_power::Target::requested(true), &mut || Ok(()))
        .unwrap();
    let clocks_on_close = Arc::new(std::sync::Mutex::new(None));
    let seen = clocks_on_close.clone();
    let observed = memory.clone();
    environment.io.0.lock().unwrap().node_hook = Some(Arc::new(move |enabled| {
        if !enabled {
            *seen.lock().unwrap() = Some(RawState::read(observed.as_ref()).unwrap());
        }
    }));
    session.recover_effects(true).unwrap();
    assert!(!environment.io.device().enabled);
    assert_ne!(clocks_on_close.lock().unwrap().as_ref().unwrap(), &raw);
    assert_eq!(RawState::read(memory.as_ref()).unwrap(), raw);
    assert!(!gpu.read_profile().available);
    session.finish().unwrap();
}

#[test]
fn linked_checkpoint_failure_has_no_clock_output_and_pre_enable_can_be_rolled_back() {
    let (memory, gpu, environment, power) = setup();
    close_node(&environment);
    let hal = gpu_power::test_fixture::hal();
    let policy = policy(&memory);
    let session = Context::with_source(&hal, &gpu, &power, Box::new(SourceFixture(memory.clone())))
        .session()
        .unwrap();
    let device = environment.io.device();
    let before = NodeState::from_device(&device).unwrap();
    session
        .freeze_node(&device, gpu_power::Target::requested(true))
        .unwrap();
    session.authorize().unwrap();
    session
        .perform(&before, gpu_power::Target::requested(true), &mut || Ok(()))
        .unwrap();
    assert!(
        session
            .prepare_clocks(
                &policy,
                3,
                true,
                false,
                &ClockObservation::PoweredOff,
                Some(ClockOrigin::AfterEnable),
                &mut |_, _, _| bail!("隔离检查点落盘失败"),
                &mut || Ok(())
            )
            .is_err()
    );
    assert!(memory.0.lock().unwrap().writes.is_empty());
    session.rollback_nodes().unwrap();
    assert!(!environment.io.device().enabled);
    session.finish().unwrap();
}

#[test]
fn skipped_resume_never_reads_source_or_replaces_the_frozen_clock_checkpoint() {
    struct NoSource;
    impl SourceIo for NoSource {
        fn read(&self) -> Result<Source> {
            panic!("跳过调频不能读取驱动来源")
        }
    }
    impl crate::powerctl::mode::links::Source for NoSource {
        fn on_ac(&self) -> Result<Option<bool>> {
            panic!("唤醒不得查询调频来源")
        }
        fn policy(&self) -> Result<Policy> {
            panic!("唤醒不得查询GPU策略")
        }
    }
    let (memory, gpu, _environment, power) = setup();
    let hal = gpu_power::test_fixture::hal();
    let policy = policy(&memory);
    let session = Context::with_source(&hal, &gpu, &power, Box::new(NoSource))
        .session()
        .unwrap();
    session
        .prepare_clocks(
            &policy,
            3,
            true,
            true,
            &ClockObservation::Untracked,
            None,
            &mut |_, _, _| panic!("跳过不能创建检查点"),
            &mut || Ok(()),
        )
        .unwrap();
    session
        .apply_clocks(
            &mut || panic!("跳过不能查询策略"),
            &mut |_, _, _| Ok(()),
            &mut || Ok(()),
        )
        .unwrap();
    assert!(
        session
            .prepare_clocks(
                &policy,
                3,
                true,
                false,
                &ClockObservation::Untracked,
                None,
                &mut |_, _, _| Ok(()),
                &mut || Ok(())
            )
            .is_err()
    );
    assert_eq!(memory.0.lock().unwrap().reads, 0);
    assert!(memory.0.lock().unwrap().writes.is_empty());
}

#[test]
fn linked_clock_preparation_rejects_a_changed_raw_preview_before_checkpoint_or_output() {
    let (memory, gpu, _environment, power) = setup();
    let hal = gpu_power::test_fixture::hal();
    let policy = policy(&memory);
    let mut before = RawState::read(memory.as_ref()).unwrap();
    before.offsets[0].core_khz = before.offsets[0].core_khz.map(|value| value + 1000);
    let session = Context::with_source(&hal, &gpu, &power, Box::new(SourceFixture(memory.clone())))
        .session()
        .unwrap();
    assert!(
        session
            .prepare_clocks(
                &policy,
                3,
                true,
                false,
                &ClockObservation::Ready(before),
                Some(ClockOrigin::BeforeMode),
                &mut |_, _, _| panic!("失效前态不能持久化"),
                &mut || Ok(())
            )
            .is_err()
    );
    assert!(memory.0.lock().unwrap().writes.is_empty());
}

#[test]
fn mode_initial_restore_replays_firmware_even_when_readable_node_flags_already_match() {
    let (_memory, gpu, environment, power) = setup();
    let hal = gpu_power::test_fixture::hal();
    let session = Context::native(&hal, &gpu, &power).session().unwrap();
    let before = NodeState::from_device(&environment.io.device()).unwrap();
    session
        .freeze_restore(Some(&before), Some(&before))
        .unwrap();
    session.authorize().unwrap();
    assert!(environment.io.0.lock().unwrap().events.is_empty());
    session
        .restore_node(Some(&before), Some(&before), &mut || Ok(()))
        .unwrap();
    assert!(
        environment
            .io
            .0
            .lock()
            .unwrap()
            .events
            .iter()
            .any(|event| event == "firmware:true")
    );
    assert_eq!(environment.io.device().target(), before.target);
    session.finish().unwrap();
    assert_eq!(environment.authorizations.load(Ordering::SeqCst), 1);
}
