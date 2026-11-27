use super::*;

fn device(enabled: bool, automatic: bool) -> Device {
    Device {
        id: "ACPI\\NVDA0820\\INSTANCE".into(),
        name: "GPU".into(),
        enabled,
        automatic_power_off: automatic,
        problem: if enabled { 0 } else { 22 },
        drives_display: false,
        can_disable: true,
    }
}

#[test]
fn enabling_occurs_before_performance_or_entertainment_and_after_quiet() {
    for automatic in [true, false] {
        let current = device(false, automatic);
        for (mode, stage) in [
            (0, Stage::AfterMode),
            (2, Stage::BeforeMode),
            (3, Stage::BeforeMode),
        ] {
            for trigger in [
                None,
                Some(Trigger::Startup),
                Some(Trigger::SupplyChange),
                Some(Trigger::Resume),
            ] {
                let action = action(true, Some(&current), mode, trigger, true)
                    .unwrap()
                    .unwrap();
                assert_eq!(action.stage, stage);
                assert_eq!(
                    action.target,
                    Target {
                        enabled: true,
                        automatic_power_off: automatic
                    }
                );
            }
        }
    }
}

#[test]
fn only_dc_mode_resume_may_automatically_power_off_in_saving_mode() {
    for trigger in [Trigger::Startup, Trigger::SupplyChange, Trigger::Resume] {
        for dc in [true, false] {
            for automatic in [true, false] {
                let current = device(true, automatic);
                let result = action(true, Some(&current), 1, Some(trigger), dc).unwrap();
                assert_eq!(
                    result.is_some(),
                    trigger == Trigger::Resume && dc && automatic
                );
                if let Some(result) = result {
                    assert_eq!(result.stage, Stage::AfterMode);
                    assert_eq!(
                        result.target,
                        Target {
                            enabled: false,
                            automatic_power_off: true
                        }
                    );
                }
            }
        }
    }
}

#[test]
fn missing_unhealthy_or_display_critical_devices_refuse_action() {
    assert!(action(true, None, 2, Some(Trigger::Startup), true).is_err());
    assert!(
        action(false, None, 2, Some(Trigger::Startup), true)
            .unwrap()
            .is_none()
    );
    let mut current = device(true, true);
    current.can_disable = false;
    assert!(action(true, Some(&current), 1, Some(Trigger::Resume), true).is_err());
    current.problem = 43;
    assert!(action(true, Some(&current), 2, Some(Trigger::SupplyChange), true).is_err());
    assert!(action(false, Some(&current), 2, Some(Trigger::Startup), true).is_err());
}

#[test]
fn explicit_selection_confirms_power_off_but_never_changes_the_automatic_preference() {
    for automatic in [true, false] {
        for enabled in [true, false] {
            let current = device(enabled, automatic);
            let result = action(true, Some(&current), 1, None, false).unwrap();
            assert_eq!(result.is_some(), automatic);
            if let Some(result) = result {
                assert_eq!(result.stage, Stage::AfterMode);
                assert!(!result.target.enabled && result.target.automatic_power_off);
                let before = State::from_device(&current).unwrap();
                validate_action(Some(result), Some(&before), 1, None).unwrap();
                assert!(
                    validate_action(Some(result), Some(&before), 1, Some(Trigger::SupplyChange))
                        .is_err()
                );
            }
        }
    }
}

#[test]
fn explicit_mode_action_and_restore_state_reject_changed_policy_or_invalid_identity() {
    let current = device(true, true);
    let before = State::from_device(&current).unwrap();
    let mut wanted = action(true, Some(&current), 1, None, false)
        .unwrap()
        .unwrap();
    wanted.target.automatic_power_off = false;
    assert!(validate_action(Some(wanted), Some(&before), 1, None).is_err());
    wanted.target.automatic_power_off = true;
    wanted.stage = Stage::BeforeMode;
    assert!(validate_action(Some(wanted), Some(&before), 1, None).is_err());
    let mut invalid = before.clone();
    invalid.name.push('\0');
    assert!(invalid.validate().is_err());
    assert!(action(true, Some(&current), 4, None, false).is_err());
}
