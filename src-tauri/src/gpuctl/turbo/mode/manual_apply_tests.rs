use super::*;

#[test]
fn manual_full_preview_is_read_only_and_confirmation_executes_the_production_gpu_and_mode_plan() {
    let f = core();
    let (memory, gpu, environment, power) = setup();
    close_node(&environment);
    environment.io.0.lock().unwrap().device.automatic_power_off = true;
    gpu.set_power_suspended(true).unwrap();
    let raw = RawState::read(memory.as_ref()).unwrap();
    let target = target(&f, &memory, &gpu, &power, 3);
    assert_eq!(target.from_mode, Some(2));
    assert!(
        target
            .gpu
            .as_ref()
            .unwrap()
            .target()
            .unwrap()
            .clocks
            .deferred_reason
            .is_some()
    );
    assert!(target.revision.is_some());
    assert!(memory.0.lock().unwrap().writes.is_empty());
    assert!(environment.io.0.lock().unwrap().events.is_empty());
    assert!(f.device.0.lock().unwrap().events.is_empty());
    mode::selection::apply_preview(context(&f, &memory, &gpu, &power), &target, &mut || Ok(()))
        .unwrap();
    assert_eq!(f.device.0.lock().unwrap().pages[1][1], 3);
    assert!(environment.io.device().enabled && environment.io.device().automatic_power_off);
    assert_ne!(RawState::read(memory.as_ref()).unwrap(), raw);
}

#[test]
fn manual_missing_or_null_gpu_confirmation_never_authorizes_or_writes() {
    let f = core();
    let (memory, gpu, environment, power) = setup();
    close_node(&environment);
    let original = target(&f, &memory, &gpu, &power, 3);
    for field in ["gpu", "fromMode", "revision", "unavailableReason"] {
        let mut json = serde_json::to_value(&original).unwrap();
        json.as_object_mut().unwrap().remove(field);
        assert!(serde_json::from_value::<mode::Target>(json).is_err());
    }
    let mut old = original;
    old.gpu = None;
    assert!(
        mode::selection::apply_preview(context(&f, &memory, &gpu, &power), &old, &mut || Ok(()))
            .is_err()
    );
    assert!(f.hal.recovery().unwrap().oem_mode.is_none());
    assert!(memory.0.lock().unwrap().writes.is_empty());
    assert!(environment.io.0.lock().unwrap().events.is_empty());
    assert_eq!(environment.authorizations.load(Ordering::SeqCst), 0);
}

#[test]
fn manual_core_front_state_changes_before_confirmation_invalidate_the_complete_revision() {
    for change in ["ac", "energy", "fan"] {
        let f = core().with_energy(2);
        let (memory, gpu, environment, power) = setup();
        close_node(&environment);
        let target = target(&f, &memory, &gpu, &power, 3);
        {
            let mut state = f.device.0.lock().unwrap();
            match change {
                "ac" => state.pages[1][16] = (state.pages[1][16] + 1) % 4,
                "energy" => state.bios[2] = 1 << 4,
                "fan" => state.pages[4][7] = 1,
                _ => unreachable!(),
            }
        }
        assert!(
            mode::selection::apply_preview(
                context(&f, &memory, &gpu, &power),
                &target,
                &mut || Ok(())
            )
            .is_err(),
            "{change}"
        );
        assert!(f.device.0.lock().unwrap().events.is_empty(), "{change}");
        assert!(environment.io.0.lock().unwrap().events.is_empty());
        assert_eq!(environment.authorizations.load(Ordering::SeqCst), 0);
    }
}

#[test]
fn manual_later_frequency_failure_restores_the_core_and_pre_enabled_node_and_keeps_a_retryable_baseline()
 {
    let f = core();
    let (memory, gpu, environment, power) = setup();
    close_node(&environment);
    let wanted = target(&f, &memory, &gpu, &power, 3);
    let raw = RawState::read(memory.as_ref()).unwrap();
    memory.0.lock().unwrap().fail_on = vec![1];
    assert!(
        mode::selection::apply_preview(context(&f, &memory, &gpu, &power), &wanted, &mut || Ok(()))
            .is_err()
    );
    assert_eq!(f.device.0.lock().unwrap().pages[1][1], 2);
    assert_eq!(RawState::read(memory.as_ref()).unwrap(), raw);
    assert!(!environment.io.device().enabled);
    assert!(f.hal.recovery().unwrap().oem_mode.is_some());
}

#[test]
fn manual_mode_policy_or_device_change_during_authorization_rejects_before_any_output() {
    for change in ["mode", "node", "policy", "supply"] {
        let f = core();
        let (memory, gpu, environment, power) = setup();
        close_node(&environment);
        let target = target(&f, &memory, &gpu, &power, 3);
        let source = memory.clone();
        let device = f.device.clone();
        let node = environment.io.clone();
        environment.io.0.lock().unwrap().authorize_hook = Some(Box::new(move || match change {
            "mode" => device.0.lock().unwrap().pages[1][1] = 0,
            "node" => node.0.lock().unwrap().device.automatic_power_off = true,
            "policy" => source.0.lock().unwrap().source.saved[0].core_mhz += 1,
            "supply" => source.0.lock().unwrap().source.on_ac = Some(false),
            _ => unreachable!(),
        }));
        assert!(
            mode::selection::apply_preview(
                context(&f, &memory, &gpu, &power),
                &target,
                &mut || Ok(())
            )
            .is_err(),
            "{change}"
        );
        assert!(f.device.0.lock().unwrap().events.is_empty());
        assert!(memory.0.lock().unwrap().writes.is_empty());
        assert!(environment.io.0.lock().unwrap().events.is_empty());
        assert_eq!(environment.authorizations.load(Ordering::SeqCst), 1);
        assert_eq!(environment.prepared.load(Ordering::SeqCst), 0);
    }
}
