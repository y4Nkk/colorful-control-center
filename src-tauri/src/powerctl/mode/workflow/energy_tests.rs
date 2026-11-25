use super::*;
#[path = "../../../profiles/apply/fixtures.rs"]
#[allow(dead_code)]
mod fixtures;
use fixtures::Fixture;

fn setup(value: u8) -> Fixture {
    Fixture::new().with_energy(value)
}
fn current(f: &Fixture) -> energy::State {
    f.hal
        .with_dchu(|io| energy::read(io, f.hal.reviewed_dsdt))
        .unwrap()
}
fn apply(f: &Fixture, wanted: u8) -> Result<()> {
    let target = super::super::read(&f.hal)?
        .targets
        .into_iter()
        .find(|target| target.mode == wanted)
        .unwrap();
    apply_preview(&f.hal, &target)
}
fn at(f: &Fixture, prefix: &str) -> usize {
    f.device
        .0
        .lock()
        .unwrap()
        .events
        .iter()
        .position(|event| event.starts_with(prefix))
        .unwrap()
}
fn options(f: &mut Fixture) {
    f.hal.set_test_dtt_driver(std::sync::Arc::new(|| Ok(true)));
    let mut state = f.device.0.lock().unwrap();
    state.pages[1][1] = 2;
    state.bios[0] = 1;
    state.bios[16] |= 16;
    state.bios[11] |= 16;
}
fn apply_options(f: &Fixture) {
    let p = boost::read(&f.hal).unwrap();
    boost::apply(
        &f.hal,
        &boost::Request {
            expected_revision: p.revision,
            quiet_fan: None,
            replace_custom_fan: false,
            values: boost::Values {
                turbo_fan: Some(true),
                dtt: Some(false),
            },
        },
    )
    .unwrap();
}

#[test]
fn all_explicit_modes_preview_and_apply_the_original_energy_values_once() {
    for (wanted, initial, expected) in [(0, 1, 2), (1, 2, 1), (2, 2, 0), (3, 1, 2), (4, 2, 1)] {
        let f = setup(initial);
        if wanted == 4 {
            let mut state = f.device.0.lock().unwrap();
            state.bios[0] = 1;
            state.bios[17] |= 0x80;
        }
        let _gate = f.hal.begin_write().unwrap();
        let profile = super::super::read(&f.hal).unwrap();
        let target = profile
            .targets
            .into_iter()
            .find(|target| target.mode == wanted)
            .unwrap();
        assert_eq!(target.energy.unwrap().energy_star, Some(expected));
        assert!(f.device.0.lock().unwrap().events.is_empty());
        apply_preview(&f.hal, &target).unwrap();
        assert_eq!(current(&f).energy_star, Some(expected));
        assert_eq!(
            f.device
                .0
                .lock()
                .unwrap()
                .events
                .iter()
                .filter(|event| event.starts_with("ENERGY:"))
                .count(),
            1
        );
        assert!(at(&f, "MODE:") < at(&f, "ENERGY:"));
        assert!(at(&f, "ENERGY:") < at(&f, "PAGE4:5:"));
    }
}

#[test]
fn permission_can_read_dchu_at_each_mode_write_stage() {
    let f = setup(2);
    {
        let mut state = f.device.0.lock().unwrap();
        state.bios[10] |= 128;
        state.bios[16] |= 16;
    }
    let _gate = f.hal.begin_write().unwrap();
    let preferences = boost::Values {
        turbo_fan: Some(true),
        dtt: None,
    };
    let plan = checkpoint_for_profile(&f.hal, 2, Some(&preferences)).unwrap();
    plan.execute_guarded(&f.hal, &mut || {
        assert!(f.hal.test_dchu_available(), "模式许可持有 DCHU 锁");
        fanctl::dust::ensure_manual_allowed(&f.hal)?;
        f.device.0.lock().unwrap().events.push("PERMIT".into());
        Ok(())
    })
    .unwrap();
    let state = f.device.0.lock().unwrap();
    let mut next = 0;
    for output in [
        "PAGE4:8:[1]",
        "PAGE1:16:[2]",
        "MODE:66",
        "PAGE1:1:[2]",
        "ENERGY:0",
    ] {
        let position = next
            + state.events[next..]
                .iter()
                .position(|event| event == output)
                .unwrap();
        assert_eq!(state.events[position - 1], "PERMIT", "{output}");
        next = position + 1;
    }
}

#[test]
fn energy_revocation_or_a_change_during_permission_prevents_output() {
    let f = setup(0);
    let _gate = f.hal.begin_write().unwrap();
    let before = current(&f);
    let desired = energy::desired(3, before).unwrap();
    let error =
        energy::apply(&f.hal, Some(before), Some(desired), &mut || bail!("撤销")).unwrap_err();
    assert_eq!(error.to_string(), "撤销");
    let error = energy::apply(&f.hal, Some(before), Some(desired), &mut || {
        let mut state = f.device.0.lock().unwrap();
        state.bios[2] |= 0x10;
        Ok(())
    })
    .unwrap_err();
    assert!(error.to_string().contains("前态已变化"));
    assert!(f.device.0.lock().unwrap().events.is_empty());
}

#[test]
fn gpu_callbacks_enclose_the_single_common_energy_step() {
    let f = setup(0);
    let _gate = f.hal.begin_write().unwrap();
    let plan = checkpoint(&f.hal, 3).unwrap();
    plan.execute_linked(
        &f.hal,
        &mut || Ok(()),
        &mut |_| {
            assert_eq!(current(&f).energy_star, Some(0));
            f.device
                .0
                .lock()
                .unwrap()
                .events
                .push("GPU_FREQUENCY".into());
            Ok(())
        },
        &mut |_| {
            assert_eq!(current(&f).energy_star, Some(2));
            f.device.0.lock().unwrap().events.push("GPU_AFTER".into());
            Ok(())
        },
        &mut || Ok(()),
    )
    .unwrap();
    assert!(at(&f, "MODE:3") < at(&f, "GPU_FREQUENCY"));
    assert!(at(&f, "GPU_FREQUENCY") < at(&f, "ENERGY:2"));
    assert!(at(&f, "ENERGY:2") < at(&f, "GPU_AFTER"));
    assert!(at(&f, "GPU_AFTER") < at(&f, "PAGE4:5:"));
}

#[test]
fn a_failed_command_or_false_ack_restores_mode_and_keeps_the_energy_baseline() {
    for ignore in [false, true] {
        let f = setup(2);
        let _gate = f.hal.begin_write().unwrap();
        {
            let mut state = f.device.0.lock().unwrap();
            state.ignore_energy_write = ignore;
            if !ignore {
                state.fail_energy = Some(1);
            }
        }
        assert!(apply(&f, 1).is_err());
        assert_eq!(f.device.0.lock().unwrap().pages[1][1], 3);
        assert_eq!(current(&f).energy_star, Some(2));
        assert_eq!(
            f.hal.recovery()
                .unwrap()
                .oem_mode
                .clone()
                .unwrap()
                .energy
                .unwrap()
                .energy_star,
            Some(2)
        );
    }
}

#[test]
fn later_fan_failure_compensates_the_common_energy_step() {
    let f = setup(1);
    let _gate = f.hal.begin_write().unwrap();
    {
        let mut state = f.device.0.lock().unwrap();
        state.bios[20] |= 0x40;
        state.fail_fan_mode = true;
    }
    assert!(apply(&f, 2).is_err());
    assert_eq!(current(&f).energy_star, Some(1));
    assert_eq!(f.device.0.lock().unwrap().pages[1][1], 3);
    assert!(at(&f, "ENERGY:0") < at(&f, "FAN_MODE_FAILURE"));
    assert!(at(&f, "FAN_MODE_FAILURE") < at(&f, "ENERGY:1"));
}

#[test]
fn changed_preflight_energy_and_unrestorable_firmware_stop_before_all_outputs() {
    let f = setup(2);
    let _gate = f.hal.begin_write().unwrap();
    let plan = checkpoint(&f.hal, 1).unwrap();
    f.device.0.lock().unwrap().bios[2] = 0;
    assert!(plan.execute(&f.hal).is_err());
    assert!(f.device.0.lock().unwrap().events.is_empty());
    f.device.0.lock().unwrap().bios[2] = 3 << 4;
    assert!(super::super::read(&f.hal).is_err());
    assert!(checkpoint(&f.hal, 1).is_err());
    assert!(f.hal.recovery().unwrap().oem_mode.is_none());
}

#[test]
fn hardware_managed_energy_never_sends_79_for_selection_or_compensation() {
    let f = Fixture::new();
    let _gate = f.hal.begin_write().unwrap();
    apply(&f, 1).unwrap();
    assert!(current(&f).hardware_p_state);
    let before = f.hal.recovery().unwrap().oem_mode.clone().unwrap();
    restore_snapshot(&f.hal, &before).unwrap();
    assert!(
        !f.device
            .0
            .lock()
            .unwrap()
            .events
            .iter()
            .any(|event| event.starts_with("ENERGY:"))
    );
}

#[test]
fn option_only_recovery_leaves_a_later_energy_value_untouched() {
    let mut f = setup(3);
    options(&mut f);
    let _gate = f.hal.begin_write().unwrap();
    apply_options(&f);
    let before = f.hal.recovery().unwrap().oem_mode.clone().unwrap();
    assert!(before.energy.is_none());
    f.device.0.lock().unwrap().bios[2] = 0;
    restore_snapshot(&f.hal, &before).unwrap();
    assert_eq!(current(&f).energy_star, Some(0));
    assert!(
        !f.device
            .0
            .lock()
            .unwrap()
            .events
            .iter()
            .any(|event| event.starts_with("ENERGY:"))
    );
}

#[test]
fn choosing_after_options_adds_only_the_first_energy_baseline() {
    let mut f = setup(3);
    options(&mut f);
    let _gate = f.hal.begin_write().unwrap();
    apply_options(&f);
    let old = f.hal.recovery().unwrap().oem_mode.clone().unwrap();
    f.device.0.lock().unwrap().bios[2] = 0;
    apply(&f, 1).unwrap();
    let baseline = f.hal.recovery().unwrap().oem_mode.clone().unwrap();
    assert_eq!(baseline.energy.unwrap().energy_star, Some(0));
    assert_eq!(
        (baseline.core.mode, baseline.core.preferences[0]),
        (old.core.mode, old.core.preferences[0])
    );
    assert!(baseline.stored() != old.stored());
    restore_snapshot(&f.hal, &baseline).unwrap();
    assert_eq!(current(&f).energy_star, Some(0));
    assert_eq!(f.device.0.lock().unwrap().pages[4][8], 0);
}

#[test]
fn changed_energy_management_blocks_every_restore_output_and_is_visible() {
    let f = setup(1);
    let _gate = f.hal.begin_write().unwrap();
    apply(&f, 2).unwrap();
    let before = f.hal.recovery().unwrap().oem_mode.clone().unwrap();
    f.device.0.lock().unwrap().bios[9] = 0x40;
    f.device.0.lock().unwrap().events.clear();
    assert!(restore_snapshot(&f.hal, &before).is_err());
    assert!(f.device.0.lock().unwrap().events.is_empty());
    assert!(f.hal.recovery().unwrap().oem_mode.is_some());
}

#[test]
fn unreadable_energy_during_compensation_does_not_skip_other_owned_domains() {
    let f = setup(2);
    let _gate = f.hal.begin_write().unwrap();
    let plan = checkpoint(&f.hal, 1).unwrap();
    let error = plan
        .execute_linked(
            &f.hal,
            &mut || Ok(()),
            &mut |_| Ok(()),
            &mut |_| {
                f.device.0.lock().unwrap().bios[2] = 3 << 4;
                bail!("隔离后续域失败")
            },
            &mut || Ok(()),
        )
        .unwrap_err();
    assert!(format!("{error:#}").contains("EnergyStar"));
    assert_eq!(f.device.0.lock().unwrap().pages[1][1], 3);
    assert_eq!(f.device.0.lock().unwrap().pages[4][5], 0);
    assert!(f.hal.recovery().unwrap().oem_mode.is_some());
    f.device.0.lock().unwrap().bios[2] = 1 << 4;
    let before = f.hal.recovery().unwrap().oem_mode.clone().unwrap();
    restore_snapshot(&f.hal, &before).unwrap();
    assert_eq!(current(&f).energy_star, Some(2));
}
