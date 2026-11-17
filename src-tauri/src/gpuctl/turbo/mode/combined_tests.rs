use super::*;
use crate::{
    gpu_power::mode as node,
    powerctl::mode::{self, links},
};
#[path = "../../../profiles/apply/fixtures.rs"]
#[allow(dead_code)]
mod fixtures;
use fixtures::Fixture;
#[path = "cycle_tests.rs"]
mod cycle_tests;
#[path = "manual_apply_tests.rs"]
mod manual_apply_tests;
#[path = "manual_restore_tests.rs"]
mod manual_restore_tests;
#[path = "profile_application_tests.rs"]
#[cfg(not(control_sim))]
mod profile_application_tests;
#[path = "profile_plan_tests.rs"]
mod profile_plan_tests;

struct CombinedSource {
    memory: Arc<Memory>,
    device: Arc<fixtures::Device>,
}
impl SourceIo for CombinedSource {
    fn read(&self) -> Result<Source> {
        let mut source = self.memory.read()?;
        source.mode = self.device.0.lock().unwrap().pages[1][1];
        Ok(source)
    }
}
impl links::Source for CombinedSource {
    fn on_ac(&self) -> Result<Option<bool>> {
        Ok(self.memory.0.lock().unwrap().source.on_ac)
    }
    fn policy(&self) -> Result<Policy> {
        Ok(policy(&self.memory))
    }
}
fn core() -> Fixture {
    let f = Fixture::new();
    {
        let mut data = f.device.0.lock().unwrap();
        data.pages[1][1] = 2;
        data.bios[0] = 1;
        data.bios[17] = 15;
    }
    f
}
fn context<'a>(
    f: &'a Fixture,
    memory: &Arc<Memory>,
    gpu: &'a crate::gpuctl::GpuController,
    power: &'a gpu_power::Controller,
) -> links::Context<'a> {
    links::Context::with_source(
        &f.hal,
        gpu,
        power,
        Box::new(CombinedSource {
            memory: memory.clone(),
            device: f.device.clone(),
        }),
    )
}
fn plan(f: &Fixture, memory: &Memory, environment: &EnvironmentFixture, wanted: u8) -> links::Plan {
    let device = environment.io.device();
    links::Plan::new(
        mode::checkpoint_for_profile(&f.hal, wanted, None).unwrap(),
        links::Effects {
            node_before: Some(node::State::from_device(&device).unwrap()),
            node_action: node::action(true, Some(&device), wanted, None, true).unwrap(),
            clock_policy: Some(policy(memory)),
            clock_before: ClockObservation::PoweredOff,
            on_ac: true,
            trigger: None,
        },
    )
    .unwrap()
}
fn full_profile(
    f: &Fixture,
    memory: &Arc<Memory>,
    gpu: &crate::gpuctl::GpuController,
    power: &gpu_power::Controller,
) -> mode::ModeProfile {
    let context = context(f, memory, gpu, power);
    let _gate = f.hal.begin_write().unwrap();
    mode::selection::read(&context).unwrap()
}
fn target(
    f: &Fixture,
    memory: &Arc<Memory>,
    gpu: &crate::gpuctl::GpuController,
    power: &gpu_power::Controller,
    wanted: u8,
) -> mode::Target {
    full_profile(f, memory, gpu, power)
        .targets
        .into_iter()
        .find(|target| target.mode == wanted)
        .unwrap()
}

#[test]
fn combined_production_mode_and_frequency_plan_preserves_checkpoint_order_and_origin() {
    let f = core();
    let (memory, gpu, environment, power) = setup();
    close_node(&environment);
    gpu.set_power_suspended(true).unwrap();
    let session = context(&f, &memory, &gpu, &power).session().unwrap();
    let gate = f.hal.begin_write().unwrap();
    let plan = plan(&f, &memory, &environment, 3);
    let raw = RawState::read(memory.as_ref()).unwrap();
    session
        .freeze_node(
            &environment.io.device(),
            plan.effects.node_action.unwrap().target,
        )
        .unwrap();
    drop(gate);
    session.authorize().unwrap();
    let gate = f.hal.begin_write().unwrap();
    let mut stages = Vec::new();
    plan.execute(
        &session,
        &mut |write, origin, _| {
            assert_eq!(origin, ClockOrigin::AfterEnable);
            assert_eq!(write.before, raw);
            assert!(memory.0.lock().unwrap().writes.is_empty());
            stages.push(f.device.0.lock().unwrap().pages[1][1]);
            Ok(())
        },
        &mut || Ok(()),
    )
    .unwrap();
    assert_eq!(stages, vec![2, 3]);
    let target = plan
        .effects
        .clock_policy
        .as_ref()
        .unwrap()
        .decide(3, Some(true), false)
        .unwrap()
        .target
        .unwrap();
    assert_eq!(
        RawState::read(memory.as_ref()).unwrap(),
        raw.projected(&target).unwrap()
    );
    assert_eq!(f.device.0.lock().unwrap().pages[1][1], 3);
    assert!(environment.io.device().enabled);
    drop(gate);
    session.finish().unwrap();
}

fn observe_close(
    f: &Fixture,
    memory: &Arc<Memory>,
    environment: &EnvironmentFixture,
) -> Arc<std::sync::Mutex<Option<(u8, RawState)>>> {
    let state = Arc::new(std::sync::Mutex::new(None));
    let observed = state.clone();
    let clocks = memory.clone();
    let device = f.device.clone();
    environment.io.0.lock().unwrap().node_hook = Some(Arc::new(move |enabled| {
        if !enabled {
            let mode = device.0.lock().unwrap().pages[1][1];
            *observed.lock().unwrap() = Some((mode, RawState::read(clocks.as_ref()).unwrap()));
        }
    }));
    state
}

#[test]
fn combined_performance_failure_restores_frequency_and_core_before_pre_enabled_node() {
    let f = core();
    let (memory, gpu, environment, power) = setup();
    close_node(&environment);
    gpu.set_power_suspended(true).unwrap();
    let session = context(&f, &memory, &gpu, &power).session().unwrap();
    let gate = f.hal.begin_write().unwrap();
    let plan = plan(&f, &memory, &environment, 3);
    let raw = RawState::read(memory.as_ref()).unwrap();
    let observed = observe_close(&f, &memory, &environment);
    session
        .freeze_node(
            &environment.io.device(),
            plan.effects.node_action.unwrap().target,
        )
        .unwrap();
    drop(gate);
    session.authorize().unwrap();
    let gate = f.hal.begin_write().unwrap();
    f.device.0.lock().unwrap().fail_fan_mode = true;
    assert!(
        plan.execute(&session, &mut |_, _, _| Ok(()), &mut || Ok(()))
            .is_err()
    );
    assert_eq!(*observed.lock().unwrap(), Some((2, raw.clone())));
    assert_eq!(RawState::read(memory.as_ref()).unwrap(), raw);
    assert!(!environment.io.device().enabled);
    assert_eq!(f.device.0.lock().unwrap().pages[1][1], 2);
    assert!(!gpu.read_profile().available);
    drop(gate);
    session.finish().unwrap();
}

#[test]
fn combined_quiet_failure_restores_post_mode_node_then_frequency_then_core() {
    let f = core();
    let (memory, gpu, environment, power) = setup();
    close_node(&environment);
    gpu.set_power_suspended(true).unwrap();
    let session = context(&f, &memory, &gpu, &power).session().unwrap();
    let gate = f.hal.begin_write().unwrap();
    let plan = plan(&f, &memory, &environment, 0);
    let raw = RawState::read(memory.as_ref()).unwrap();
    let observed = observe_close(&f, &memory, &environment);
    session
        .freeze_node(
            &environment.io.device(),
            plan.effects.node_action.unwrap().target,
        )
        .unwrap();
    drop(gate);
    session.authorize().unwrap();
    let gate = f.hal.begin_write().unwrap();
    f.device.0.lock().unwrap().fail_fan_mode = true;
    assert!(
        plan.execute(
            &session,
            &mut |_, origin, _| {
                assert_eq!(origin, ClockOrigin::PoweredOff);
                Ok(())
            },
            &mut || Ok(())
        )
        .is_err()
    );
    let close = observed.lock().unwrap().clone().unwrap();
    assert_eq!(close.0, 0);
    assert_ne!(close.1, raw);
    assert_eq!(RawState::read(memory.as_ref()).unwrap(), raw);
    assert!(!environment.io.device().enabled);
    assert_eq!(f.device.0.lock().unwrap().pages[1][1], 2);
    assert!(!gpu.read_profile().available);
    drop(gate);
    session.finish().unwrap();
}
