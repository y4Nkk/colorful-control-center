use super::*;
use mode::profile;

#[test]
fn a_reused_plan_rejected_before_output_does_not_retain_a_previous_attempt_marker() {
    let f = core();
    let (memory, gpu, _environment, power) = setup();
    let session = context(&f, &memory, &gpu, &power).session().unwrap();
    let gate = f.hal.begin_write().unwrap();
    let plan = profile::checkpoint(&session.context, 3, None, None, false).unwrap();
    session
        .freeze_requests(profile::stages(&plan).unwrap().unwrap().requests())
        .unwrap();
    drop(gate);
    session.authorize().unwrap();
    let _gate = f.hal.begin_write().unwrap();
    plan.execute(&session, &mut |_, _, _| Ok(()), &mut || Ok(()))
        .unwrap();
    assert!(plan.core.attempted());
    f.device.0.lock().unwrap().pages[1][1] = 0;
    let events = f.device.0.lock().unwrap().events.len();
    let writes = memory.0.lock().unwrap().writes.len();
    assert!(
        plan.execute(&session, &mut |_, _, _| Ok(()), &mut || Ok(()))
            .is_err()
    );
    assert!(!plan.core.attempted());
    assert_eq!(f.device.0.lock().unwrap().events.len(), events);
    assert_eq!(memory.0.lock().unwrap().writes.len(), writes);
}

fn wanted(environment: &EnvironmentFixture) -> gpu_power::profile::Target {
    let before = environment.io.device();
    gpu_power::profile::Target {
        id: before.id,
        name: before.name,
        values: gpu_power::Target {
            enabled: true,
            automatic_power_off: false,
        },
    }
}
fn initial(session: &links::Session<'_>, target: &gpu_power::profile::Target) -> links::Plan {
    let _gate = session.context.hal.begin_write().unwrap();
    profile::checkpoint(&session.context, 1, None, Some(target), true).unwrap()
}

#[test]
fn production_profile_mode_executes_three_node_stages_with_one_authorization_and_original_restore()
{
    let f = core().with_energy(2);
    let (memory, gpu, environment, power) = setup();
    close_node(&environment);
    environment.io.0.lock().unwrap().device.automatic_power_off = true;
    gpu.set_power_suspended(true).unwrap();
    let raw = RawState::read(memory.as_ref()).unwrap();
    let session = context(&f, &memory, &gpu, &power).session().unwrap();
    let initial = initial(&session, &wanted(&environment));
    let graph = profile::stages(&initial).unwrap().unwrap();
    assert_eq!(graph.requests().len(), 3);
    assert!(memory.0.lock().unwrap().writes.is_empty());
    session.freeze_requests(graph.requests()).unwrap();
    session.authorize().unwrap();
    let gate = f.hal.begin_write().unwrap();
    profile::prepare(&session, &initial, &mut || Ok(())).unwrap();
    let active = profile::rebase(&session.context, &initial).unwrap();
    assert_eq!(active.target, initial.target);
    assert_eq!(active.clock_origin, Some(ClockOrigin::AfterEnable));
    assert!(active.effects.node_before.as_ref().unwrap().target.enabled);
    assert!(!active.initial_node.as_ref().unwrap().target.enabled);
    active
        .execute(
            &session,
            &mut |_, origin, _| {
                assert_eq!(origin, ClockOrigin::AfterEnable);
                Ok(())
            },
            &mut || Ok(()),
        )
        .unwrap();
    assert!(!environment.io.device().enabled);
    profile::finish(&session, &active, true, &mut || Ok(())).unwrap();
    assert!(environment.io.device().enabled && !environment.io.device().automatic_power_off);
    drop(gate);
    session.finish().unwrap();
    drop(session);
    let restore = full_profile(&f, &memory, &gpu, &power).restore.unwrap();
    let gpu_initial = restore.gpu.as_ref().unwrap();
    assert!(!gpu_initial.node.as_ref().unwrap().target.enabled);
    assert!(
        gpu_initial
            .node
            .as_ref()
            .unwrap()
            .target
            .automatic_power_off
    );
    assert_eq!(
        gpu_initial.clocks.as_ref().unwrap().origin,
        ClockOrigin::AfterEnable
    );
    assert_eq!(gpu_initial.clocks.as_ref().unwrap().state, raw);
    assert_eq!(environment.authorizations.load(Ordering::SeqCst), 1);
    mode::restoration::restore(
        context(&f, &memory, &gpu, &power),
        &restore.revision,
        &mut || Ok(()),
    )
    .unwrap();
    assert!(!environment.io.device().enabled && environment.io.device().automatic_power_off);
    assert_eq!(RawState::read(memory.as_ref()).unwrap(), raw);
    assert_eq!(f.device.0.lock().unwrap().pages[1][1], 2);
}

#[test]
fn profile_preparation_cannot_hide_external_core_changes_or_create_a_mode_baseline() {
    let f = core();
    let (memory, gpu, environment, power) = setup();
    close_node(&environment);
    environment.io.0.lock().unwrap().device.automatic_power_off = true;
    gpu.set_power_suspended(true).unwrap();
    let session = context(&f, &memory, &gpu, &power).session().unwrap();
    let initial = initial(&session, &wanted(&environment));
    session
        .freeze_requests(profile::stages(&initial).unwrap().unwrap().requests())
        .unwrap();
    session.authorize().unwrap();
    let gate = f.hal.begin_write().unwrap();
    profile::prepare(&session, &initial, &mut || Ok(())).unwrap();
    f.device.0.lock().unwrap().pages[1][16] = 3;
    assert!(profile::rebase(&session.context, &initial).is_err());
    session.rollback_nodes().unwrap();
    assert!(!environment.io.device().enabled);
    assert!(f.device.0.lock().unwrap().events.is_empty());
    assert!(memory.0.lock().unwrap().writes.is_empty());
    assert!(f.hal.recovery().unwrap().oem_mode.is_none());
    assert_eq!(f.device.0.lock().unwrap().pages[1][16], 3);
    drop(gate);
    session.finish().unwrap();
}

#[test]
fn changed_display_qualification_after_preparation_rejects_before_linked_mode_and_clocks() {
    let f = core();
    let (memory, gpu, environment, power) = setup();
    close_node(&environment);
    environment.io.0.lock().unwrap().device.automatic_power_off = true;
    gpu.set_power_suspended(true).unwrap();
    let session = context(&f, &memory, &gpu, &power).session().unwrap();
    let initial = initial(&session, &wanted(&environment));
    session
        .freeze_requests(profile::stages(&initial).unwrap().unwrap().requests())
        .unwrap();
    session.authorize().unwrap();
    let gate = f.hal.begin_write().unwrap();
    profile::prepare(&session, &initial, &mut || Ok(())).unwrap();
    let active = profile::rebase(&session.context, &initial).unwrap();
    environment.io.0.lock().unwrap().device.drives_display = true;
    assert!(
        active
            .execute(&session, &mut |_, _, _| Ok(()), &mut || Ok(()))
            .is_err()
    );
    assert!(f.device.0.lock().unwrap().events.is_empty());
    assert!(memory.0.lock().unwrap().writes.is_empty());
    assert!(f.hal.recovery().unwrap().oem_mode.is_none());
    environment.io.0.lock().unwrap().device.drives_display = false;
    session.rollback_nodes().unwrap();
    drop(gate);
    session.finish().unwrap();
}
