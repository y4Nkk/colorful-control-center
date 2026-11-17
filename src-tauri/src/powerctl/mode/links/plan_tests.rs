use super::*;
use crate::{
    gpu_power::{self, mode as node, test_fixture::EnvironmentFixture},
    gpuctl::turbo::mode as clocks,
    powerctl::mode::links::{Context, Effects, Plan as LinkedPlan},
};
use ccc_core::oem_supply_mode::Trigger;

fn environment(f: &Fixture, enabled: bool) -> std::sync::Arc<EnvironmentFixture> {
    let environment = EnvironmentFixture::new();
    let observed = f.device.clone();
    {
        let mut data = environment.io.0.lock().unwrap();
        data.device.enabled = enabled;
        data.device.problem = if enabled { 0 } else { 22 };
        data.firmware = enabled;
        data.device.automatic_power_off = true;
        data.node_hook = Some(std::sync::Arc::new(move |enabled| {
            observed
                .0
                .lock()
                .unwrap()
                .events
                .push(format!("NODE:{enabled}"));
        }));
    }
    environment
}
struct SourceFixture;
impl crate::gpuctl::turbo::source::SourceIo for SourceFixture {
    fn read(&self) -> Result<crate::gpuctl::turbo::source::Source> {
        panic!("跳过调频不能读取驱动来源")
    }
}
impl crate::powerctl::mode::links::Source for SourceFixture {
    fn on_ac(&self) -> Result<Option<bool>> {
        Ok(Some(true))
    }
    fn policy(&self) -> Result<clocks::Policy> {
        Ok(clocks::test_policy())
    }
}
fn context<'a>(f: &'a Fixture, power: &'a gpu_power::Controller) -> Context<'a> {
    Context::with_source(&f.hal, &f.gpu, power, Box::new(SourceFixture))
}
struct ChangedPolicy;
impl crate::gpuctl::turbo::source::SourceIo for ChangedPolicy {
    fn read(&self) -> Result<crate::gpuctl::turbo::source::Source> {
        panic!("策略失效后不得查询驱动")
    }
}
impl crate::powerctl::mode::links::Source for ChangedPolicy {
    fn on_ac(&self) -> Result<Option<bool>> {
        Ok(Some(true))
    }
    fn policy(&self) -> Result<clocks::Policy> {
        let mut policy = clocks::test_policy();
        policy.ready = !policy.ready;
        Ok(policy)
    }
}
struct NoPolicy;
impl crate::gpuctl::turbo::source::SourceIo for NoPolicy {
    fn read(&self) -> Result<crate::gpuctl::turbo::source::Source> {
        panic!("唤醒不得查询驱动来源")
    }
}
impl crate::powerctl::mode::links::Source for NoPolicy {
    fn on_ac(&self) -> Result<Option<bool>> {
        panic!("唤醒不得查询调频来源")
    }
    fn policy(&self) -> Result<clocks::Policy> {
        panic!("唤醒不得查询调频策略")
    }
}
fn effects(environment: &EnvironmentFixture, mode: u8, trigger: Option<Trigger>) -> Effects {
    let policy = clocks::test_policy();
    let reason = if trigger == Some(Trigger::Resume) {
        ccc_core::gpu_turbo::mode::SkipReason::Resume
    } else {
        policy
            .decide(mode, Some(true), false)
            .unwrap()
            .skipped
            .unwrap()
    };
    let device = environment.io.device();
    Effects {
        node_before: Some(node::State::from_device(&device).unwrap()),
        node_action: node::action(true, Some(&device), mode, trigger, true).unwrap(),
        clock_policy: (trigger != Some(Trigger::Resume)).then_some(policy),
        clock_before: clocks::ClockObservation::Skipped(reason),
        on_ac: true,
        trigger,
    }
}
fn freeze(
    session: &crate::powerctl::mode::links::Session<'_>,
    environment: &EnvironmentFixture,
    effects: &Effects,
) {
    if let Some(action) = effects.node_action {
        session
            .freeze_node(&environment.io.device(), action.target)
            .unwrap();
    }
}
fn last_position(f: &Fixture, event: &str) -> usize {
    f.device
        .0
        .lock()
        .unwrap()
        .events
        .iter()
        .rposition(|value| value == event)
        .unwrap()
}

#[test]
fn shared_plan_pre_enables_then_restores_core_before_closing_on_a_later_failure() {
    let f = setup();
    let environment = environment(&f, false);
    let power = gpu_power::Controller::with_environment(environment.clone());
    let session = context(&f, &power).session().unwrap();
    let gate = f.hal.begin_write().unwrap();
    let effects = effects(&environment, 3, None);
    let plan = LinkedPlan::new(checkpoint(&f.hal, 3).unwrap(), effects).unwrap();
    freeze(&session, &environment, &plan.effects);
    drop(gate);
    session.authorize().unwrap();
    let gate = f.hal.begin_write().unwrap();
    f.device.0.lock().unwrap().fail_fan_mode = true;
    assert!(
        plan.execute(
            &session,
            &mut |_, _, _| panic!("无调频目标不得建立检查点"),
            &mut || Ok(())
        )
        .is_err()
    );
    assert!(position(&f, "NODE:true") < position(&f, "MODE:3"));
    assert!(last_position(&f, "MODE:2") < position(&f, "NODE:false"));
    assert!(!environment.io.device().enabled);
    assert_eq!(f.device.0.lock().unwrap().pages[1][1], 2);
    drop(gate);
    session.finish().unwrap();
}

#[test]
fn shared_plan_quiet_enables_after_mode_and_restores_the_node_before_core_on_failure() {
    let f = setup();
    let environment = environment(&f, false);
    let power = gpu_power::Controller::with_environment(environment.clone());
    let session = context(&f, &power).session().unwrap();
    let gate = f.hal.begin_write().unwrap();
    let plan = LinkedPlan::new(
        checkpoint(&f.hal, 0).unwrap(),
        effects(&environment, 0, None),
    )
    .unwrap();
    freeze(&session, &environment, &plan.effects);
    drop(gate);
    session.authorize().unwrap();
    let gate = f.hal.begin_write().unwrap();
    f.device.0.lock().unwrap().fail_fan_mode = true;
    assert!(
        plan.execute(&session, &mut |_, _, _| Ok(()), &mut || Ok(()))
            .is_err()
    );
    assert!(position(&f, "MODE:0") < position(&f, "NODE:true"));
    assert!(position(&f, "NODE:false") < last_position(&f, "MODE:2"));
    assert!(!environment.io.device().enabled);
    drop(gate);
    session.finish().unwrap();
}

#[test]
fn shared_plan_revocation_after_pre_enable_prevents_mode_output_and_releases_node() {
    let f = setup();
    let environment = environment(&f, false);
    let power = gpu_power::Controller::with_environment(environment.clone());
    let session = context(&f, &power).session().unwrap();
    let gate = f.hal.begin_write().unwrap();
    let plan = LinkedPlan::new(
        checkpoint(&f.hal, 3).unwrap(),
        effects(&environment, 3, None),
    )
    .unwrap();
    freeze(&session, &environment, &plan.effects);
    drop(gate);
    session.authorize().unwrap();
    let gate = f.hal.begin_write().unwrap();
    assert!(
        plan.execute(&session, &mut |_, _, _| Ok(()), &mut || {
            if environment.io.device().enabled {
                bail!("隔离模式许可撤销");
            }
            Ok(())
        })
        .is_err()
    );
    assert!(
        !f.device
            .0
            .lock()
            .unwrap()
            .events
            .iter()
            .any(|event| event.starts_with("MODE:"))
    );
    assert!(!environment.io.device().enabled);
    assert!(f.hal.recovery().unwrap().oem_mode.is_none());
    drop(gate);
    session.finish().unwrap();
}

#[test]
fn shared_plan_rechecks_core_and_node_after_authorization_before_any_output() {
    let f = setup();
    let environment = environment(&f, false);
    let power = gpu_power::Controller::with_environment(environment.clone());
    let session = context(&f, &power).session().unwrap();
    let gate = f.hal.begin_write().unwrap();
    let plan = LinkedPlan::new(
        checkpoint(&f.hal, 3).unwrap(),
        effects(&environment, 3, None),
    )
    .unwrap();
    freeze(&session, &environment, &plan.effects);
    drop(gate);
    session.authorize().unwrap();
    let gate = f.hal.begin_write().unwrap();
    f.device.0.lock().unwrap().pages[1][16] = 1;
    assert!(
        plan.execute(&session, &mut |_, _, _| Ok(()), &mut || Ok(()))
            .is_err()
    );
    assert!(environment.io.0.lock().unwrap().events.is_empty());
    assert!(f.device.0.lock().unwrap().events.is_empty());
    f.device.0.lock().unwrap().pages[1][16] = plan.core.before.core.ac_mode.unwrap();
    environment.io.0.lock().unwrap().device.automatic_power_off = false;
    assert!(
        plan.execute(&session, &mut |_, _, _| Ok(()), &mut || Ok(()))
            .is_err()
    );
    assert!(environment.io.0.lock().unwrap().events.is_empty());
    drop(gate);
    session.finish().unwrap();
}

#[test]
fn shared_resume_uses_no_frequency_policy_and_keeps_the_automatic_off_preference() {
    let f = setup();
    let environment = environment(&f, true);
    let power = gpu_power::Controller::with_environment(environment.clone());
    let session = Context::with_source(&f.hal, &f.gpu, &power, Box::new(NoPolicy))
        .session()
        .unwrap();
    let gate = f.hal.begin_write().unwrap();
    let plan = LinkedPlan::new(
        checkpoint_for_supply(&f.hal, 1, false).unwrap(),
        effects(&environment, 1, Some(Trigger::Resume)),
    )
    .unwrap();
    freeze(&session, &environment, &plan.effects);
    drop(gate);
    session.authorize().unwrap();
    let gate = f.hal.begin_write().unwrap();
    plan.execute(
        &session,
        &mut |_, _, _| panic!("唤醒不得调频"),
        &mut || Ok(()),
    )
    .unwrap();
    assert!(position(&f, "MODE:1") < position(&f, "NODE:false"));
    assert!(!environment.io.device().enabled && environment.io.device().automatic_power_off);
    drop(gate);
    session.finish().unwrap();
}

#[test]
fn shared_plan_rejects_untracked_frequency_and_invalid_node_stage() {
    let f = setup();
    let environment = environment(&f, false);
    let gate = f.hal.begin_write().unwrap();
    let mut invalid = effects(&environment, 3, None);
    invalid.clock_before = clocks::ClockObservation::Untracked;
    assert!(LinkedPlan::new(checkpoint(&f.hal, 3).unwrap(), invalid).is_err());
    let mut invalid = effects(&environment, 3, None);
    invalid.node_action.as_mut().unwrap().stage = node::Stage::AfterMode;
    assert!(LinkedPlan::new(checkpoint(&f.hal, 3).unwrap(), invalid).is_err());
    assert!(environment.io.0.lock().unwrap().events.is_empty());
    assert!(f.device.0.lock().unwrap().events.is_empty());
    drop(gate);
}

#[test]
fn shared_plan_changed_frequency_policy_rejects_before_pre_enable_or_core_output() {
    let f = setup();
    let environment = environment(&f, false);
    let power = gpu_power::Controller::with_environment(environment.clone());
    let session = Context::with_source(&f.hal, &f.gpu, &power, Box::new(ChangedPolicy))
        .session()
        .unwrap();
    let gate = f.hal.begin_write().unwrap();
    let plan = LinkedPlan::new(
        checkpoint(&f.hal, 3).unwrap(),
        effects(&environment, 3, None),
    )
    .unwrap();
    freeze(&session, &environment, &plan.effects);
    drop(gate);
    session.authorize().unwrap();
    let gate = f.hal.begin_write().unwrap();
    assert!(
        plan.execute(
            &session,
            &mut |_, _, _| panic!("失效策略不能创建检查点"),
            &mut || Ok(())
        )
        .is_err()
    );
    assert!(environment.io.0.lock().unwrap().events.is_empty());
    assert!(f.device.0.lock().unwrap().events.is_empty());
    drop(gate);
    session.finish().unwrap();
}

#[test]
fn quiet_plus_settle_revocation_restores_core_without_running_later_effects() {
    let f = setup();
    f.device.0.lock().unwrap().bios[17] = 0x8f;
    let environment = environment(&f, false);
    let power = gpu_power::Controller::with_environment(environment.clone());
    let session = context(&f, &power).session().unwrap();
    let gate = f.hal.begin_write().unwrap();
    let plan = LinkedPlan::new(
        checkpoint(&f.hal, 4).unwrap(),
        effects(&environment, 4, None),
    )
    .unwrap();
    freeze(&session, &environment, &plan.effects);
    drop(gate);
    session.authorize().unwrap();
    let gate = f.hal.begin_write().unwrap();
    let mut checks = 0;
    let error = plan
        .execute(
            &session,
            &mut |_, _, _| panic!("静音增强不能调频"),
            &mut || {
                if f.device.0.lock().unwrap().pages[1][1] == 4 {
                    checks += 1;
                    if checks == 3 {
                        bail!("隔离静音增强等待撤销");
                    }
                }
                Ok(())
            },
        )
        .unwrap_err();
    assert!(error.to_string().contains("隔离静音增强等待撤销"));
    assert_eq!(checks, 3);
    assert!(position(&f, "MODE:0") < last_position(&f, "MODE:2"));
    assert_eq!(f.device.0.lock().unwrap().pages[1][1], 2);
    assert!(environment.io.0.lock().unwrap().events.is_empty());
    drop(gate);
    session.finish().unwrap();
}
