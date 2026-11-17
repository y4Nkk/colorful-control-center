use super::*;

fn enable_dtt(f: &mut Fixture) {
    f.hal.set_test_dtt_driver(Arc::new(|| Ok(true)));
    f.device.0.lock().unwrap().bios[11] |= 16;
    let _gate = f.hal.begin_write().unwrap();
    let profile = mode::boost::read(&f.hal).unwrap();
    mode::boost::apply(
        &f.hal,
        &mode::boost::Request {
            expected_revision: profile.revision,
            values: mode::boost::Values {
                turbo_fan: None,
                dtt: Some(true),
            },
        },
    )
    .unwrap();
}

#[test]
fn already_matching_on_node_still_binds_display_qualification_before_any_linked_restore_output() {
    let f = core();
    let (memory, gpu, environment, power) = setup();
    environment.io.0.lock().unwrap().device.automatic_power_off = true;
    let wanted = target(&f, &memory, &gpu, &power, 1);
    mode::selection::apply_preview(context(&f, &memory, &gpu, &power), &wanted, &mut || Ok(()))
        .unwrap();
    assert!(!environment.io.device().enabled);
    {
        let mut data = environment.io.0.lock().unwrap();
        data.device.enabled = true;
        data.device.problem = 0;
        data.firmware = true;
    }
    power.read_runtime(&f.hal, &gpu).unwrap();
    let initial = full_profile(&f, &memory, &gpu, &power).restore.unwrap();
    assert!(
        initial
            .gpu
            .as_ref()
            .unwrap()
            .node
            .as_ref()
            .unwrap()
            .target
            .enabled
    );
    let core_events = f.device.0.lock().unwrap().events.clone();
    let gpu_events = memory.0.lock().unwrap().writes.clone();
    let node_events = environment.io.0.lock().unwrap().events.clone();
    let node = environment.io.clone();
    environment.io.0.lock().unwrap().authorize_hook = Some(Box::new(move || {
        let mut data = node.0.lock().unwrap();
        data.device.drives_display = true;
        data.device.can_disable = false;
    }));
    assert!(
        mode::restoration::restore(
            context(&f, &memory, &gpu, &power),
            &initial.revision,
            &mut || Ok(())
        )
        .is_err()
    );
    assert_eq!(f.device.0.lock().unwrap().events, core_events);
    assert_eq!(memory.0.lock().unwrap().writes, gpu_events);
    assert_eq!(environment.io.0.lock().unwrap().events, node_events);
    assert!(f.hal.recovery().unwrap().oem_mode.is_some());
}

#[test]
fn manual_gpu_initial_values_extend_older_option_and_energy_ownership_without_resetting_it() {
    let mut f = core().with_energy(2);
    let (memory, gpu, environment, power) = setup();
    close_node(&environment);
    let raw = RawState::read(memory.as_ref()).unwrap();
    enable_dtt(&mut f);
    assert_eq!(f.device.0.lock().unwrap().pages[1][32], 1);
    let wanted = target(&f, &memory, &gpu, &power, 3);
    mode::selection::apply_preview(context(&f, &memory, &gpu, &power), &wanted, &mut || Ok(()))
        .unwrap();
    let initial = full_profile(&f, &memory, &gpu, &power).restore.unwrap();
    assert!(!initial.dtt);
    assert_eq!(initial.energy.unwrap().energy_star, Some(2));
    assert_eq!(
        initial.gpu.as_ref().unwrap().clocks.as_ref().unwrap().state,
        raw
    );
    {
        let context = context(&f, &memory, &gpu, &power);
        let _gate = f.hal.begin_write().unwrap();
        let options = mode::selection::read_options(&context).unwrap();
        assert_eq!(options.restore.unwrap().revision, initial.revision);
    }
    mode::restoration::restore(
        context(&f, &memory, &gpu, &power),
        &initial.revision,
        &mut || Ok(()),
    )
    .unwrap();
    assert_eq!(f.device.0.lock().unwrap().pages[1][32], 0);
    assert_eq!(f.device.0.lock().unwrap().pages[1][1], 2);
    assert_eq!(RawState::read(memory.as_ref()).unwrap(), raw);
    assert!(!environment.io.device().enabled);
}

#[test]
fn core_only_option_restore_never_loads_an_uninjected_mode_gpu_provider_or_requests_authorization()
{
    let mut f = core();
    enable_dtt(&mut f);
    let environment = EnvironmentFixture::new();
    let power = gpu_power::Controller::with_environment(environment.clone());
    let gpu = crate::gpuctl::GpuController::new();
    let initial = {
        let context = links::Context::native(&f.hal, &gpu, &power);
        let _gate = f.hal.begin_write().unwrap();
        let options = mode::selection::read_options(&context).unwrap();
        assert!(options.can_restore);
        let initial = options.restore.unwrap();
        assert!(initial.gpu.is_none());
        initial
    };
    mode::restoration::restore(
        links::Context::native(&f.hal, &gpu, &power),
        &initial.revision,
        &mut || Ok(()),
    )
    .unwrap();
    assert!(f.hal.recovery().unwrap().oem_mode.is_none());
    assert_eq!(f.device.0.lock().unwrap().pages[1][32], 0);
    assert_eq!(environment.authorizations.load(Ordering::SeqCst), 0);
    assert!(environment.io.0.lock().unwrap().events.is_empty());
}

#[test]
fn manual_initial_restore_includes_frequency_and_original_closed_node_with_one_authorization() {
    let f = core();
    let (memory, gpu, environment, power) = setup();
    close_node(&environment);
    environment.io.0.lock().unwrap().device.automatic_power_off = true;
    gpu.set_power_suspended(true).unwrap();
    let raw = RawState::read(memory.as_ref()).unwrap();
    let ac = f.device.0.lock().unwrap().pages[1][16];
    let wanted = target(&f, &memory, &gpu, &power, 3);
    mode::selection::apply_preview(context(&f, &memory, &gpu, &power), &wanted, &mut || Ok(()))
        .unwrap();
    let view = full_profile(&f, &memory, &gpu, &power);
    assert!(view.can_restore, "{:?}", view.restore_blocked_reason);
    let restored = view.restore.unwrap();
    assert!(restored.current_gpu_driver_identity.is_some());
    mode::restoration::restore(
        context(&f, &memory, &gpu, &power),
        &restored.revision,
        &mut || Ok(()),
    )
    .unwrap();
    assert_eq!(RawState::read(memory.as_ref()).unwrap(), raw);
    assert!(!environment.io.device().enabled && environment.io.device().automatic_power_off);
    assert_eq!(f.device.0.lock().unwrap().pages[1][1], 2);
    assert_eq!(f.device.0.lock().unwrap().pages[1][16], ac);
    assert!(f.hal.recovery().unwrap().oem_mode.is_none());
    assert_eq!(environment.authorizations.load(Ordering::SeqCst), 2);
}

#[test]
fn manual_restore_rejects_stale_driver_range_before_node_authorization_or_core_output() {
    let f = core();
    let (memory, gpu, environment, power) = setup();
    close_node(&environment);
    let wanted = target(&f, &memory, &gpu, &power, 3);
    mode::selection::apply_preview(context(&f, &memory, &gpu, &power), &wanted, &mut || Ok(()))
        .unwrap();
    let initial = full_profile(&f, &memory, &gpu, &power).restore.unwrap();
    let core_events = f.device.0.lock().unwrap().events.clone();
    let gpu_events = memory.0.lock().unwrap().writes.clone();
    let node_events = environment.io.0.lock().unwrap().events.clone();
    memory.0.lock().unwrap().offsets[0].core.max_khz -= 1;
    let changed = full_profile(&f, &memory, &gpu, &power).restore.unwrap();
    assert_ne!(changed.revision, initial.revision);
    assert!(
        mode::restoration::restore(
            context(&f, &memory, &gpu, &power),
            &initial.revision,
            &mut || Ok(())
        )
        .is_err()
    );
    assert_eq!(environment.authorizations.load(Ordering::SeqCst), 1);
    assert_eq!(f.device.0.lock().unwrap().events, core_events);
    assert_eq!(memory.0.lock().unwrap().writes, gpu_events);
    assert_eq!(environment.io.0.lock().unwrap().events, node_events);
    memory.0.lock().unwrap().offsets[0].core.max_khz = 5_000;
    let blocked = full_profile(&f, &memory, &gpu, &power);
    assert!(!blocked.can_restore);
    assert!(blocked.restore_blocked_reason.is_some());
    assert!(blocked.restore.unwrap().gpu.is_some());
}

#[test]
fn manual_restore_rechecks_node_after_authorization_and_retains_the_first_snapshot() {
    let f = core();
    let (memory, gpu, environment, power) = setup();
    close_node(&environment);
    let wanted = target(&f, &memory, &gpu, &power, 3);
    mode::selection::apply_preview(context(&f, &memory, &gpu, &power), &wanted, &mut || Ok(()))
        .unwrap();
    let initial = full_profile(&f, &memory, &gpu, &power).restore.unwrap();
    let core_events = f.device.0.lock().unwrap().events.clone();
    let gpu_events = memory.0.lock().unwrap().writes.clone();
    let node = environment.io.clone();
    environment.io.0.lock().unwrap().authorize_hook = Some(Box::new(move || {
        node.0.lock().unwrap().device.drives_display = true;
        node.0.lock().unwrap().device.can_disable = false;
    }));
    assert!(
        mode::restoration::restore(
            context(&f, &memory, &gpu, &power),
            &initial.revision,
            &mut || Ok(())
        )
        .is_err()
    );
    assert_eq!(f.device.0.lock().unwrap().events, core_events);
    assert_eq!(memory.0.lock().unwrap().writes, gpu_events);
    assert!(f.hal.recovery().unwrap().oem_mode.is_some());
}

#[test]
fn manual_partial_frequency_restore_keeps_the_first_snapshot_and_requires_a_fresh_preview_to_retry()
{
    let f = core();
    let (memory, gpu, environment, power) = setup();
    close_node(&environment);
    let raw = RawState::read(memory.as_ref()).unwrap();
    let wanted = target(&f, &memory, &gpu, &power, 3);
    mode::selection::apply_preview(context(&f, &memory, &gpu, &power), &wanted, &mut || Ok(()))
        .unwrap();
    let first = full_profile(&f, &memory, &gpu, &power).restore.unwrap();
    {
        let mut data = memory.0.lock().unwrap();
        let next = data.writes.len() + 1;
        data.fail_on = vec![next];
    }
    assert!(
        mode::restoration::restore(
            context(&f, &memory, &gpu, &power),
            &first.revision,
            &mut || Ok(())
        )
        .is_err()
    );
    let view = full_profile(&f, &memory, &gpu, &power);
    assert!(view.can_restore, "{:?}", view.restore_blocked_reason);
    let fresh = view.restore.unwrap();
    assert_ne!(fresh.revision, first.revision);
    assert_eq!(
        fresh.gpu.as_ref().unwrap().clocks.as_ref().unwrap().state,
        raw
    );
    let authorizations = environment.authorizations.load(Ordering::SeqCst);
    assert!(
        mode::restoration::restore(
            context(&f, &memory, &gpu, &power),
            &first.revision,
            &mut || Ok(())
        )
        .is_err()
    );
    assert_eq!(
        environment.authorizations.load(Ordering::SeqCst),
        authorizations
    );
    mode::restoration::restore(
        context(&f, &memory, &gpu, &power),
        &fresh.revision,
        &mut || Ok(()),
    )
    .unwrap();
    assert_eq!(RawState::read(memory.as_ref()).unwrap(), raw);
    assert!(!environment.io.device().enabled);
    assert!(f.hal.recovery().unwrap().oem_mode.is_none());
}
