use super::*;
use ccc_core::oem_events::Action;

#[test]
fn low_temperature_query_has_its_own_capability_bound_authorization() {
    assert_eq!(
        Scope::for_action(Action::BatteryColdWarning),
        Scope::BatteryColdWarning
    );
    let mut capabilities = facts();
    let settings = Settings {
        enabled: true,
        actions: vec![Scope::StateNotifications],
        osd: true,
    };
    let notices = Record::authorize(settings, &capabilities).unwrap();
    assert!(
        !notices
            .settings
            .actions
            .contains(&Scope::BatteryColdWarning)
    );
    let settings = Settings {
        enabled: true,
        actions: vec![Scope::BatteryColdWarning],
        osd: false,
    };
    let cold = Record::authorize(settings, &capabilities).unwrap();
    cold.verify(&capabilities).unwrap();
    let selected = capabilities
        .capabilities
        .iter_mut()
        .find(|cap| cap.scope == Scope::BatteryColdWarning)
        .unwrap();
    selected.identity = Some("changed-ui-rule".into());
    assert!(cold.verify(&capabilities).is_err());
}

#[test]
fn low_battery_brightness_requires_its_own_authorization() {
    for code in [59, 60] {
        assert_eq!(
            Scope::for_action(Action::BatteryLowBrightness(code)),
            Scope::BatteryLowBrightness
        );
    }
    let mut capabilities = facts();
    let other = Record::authorize(
        Settings {
            enabled: true,
            actions: vec![Scope::ScreenBrightness],
            osd: false,
        },
        &capabilities,
    )
    .unwrap();
    assert!(
        !other
            .settings
            .actions
            .contains(&Scope::BatteryLowBrightness)
    );
    let low = Record::authorize(
        Settings {
            enabled: true,
            actions: vec![Scope::BatteryLowBrightness],
            osd: false,
        },
        &capabilities,
    )
    .unwrap();
    low.verify(&capabilities).unwrap();
    capabilities
        .capabilities
        .iter_mut()
        .find(|cap| cap.scope == Scope::BatteryLowBrightness)
        .unwrap()
        .identity = Some("changed-panel".into());
    assert!(low.verify(&capabilities).is_err());
}

#[test]
fn low_battery_system_action_requires_a_separate_plan_bound_authorization() {
    for code in [44, 45] {
        assert_eq!(
            Scope::for_action(Action::BatteryLowAction(code)),
            Scope::BatteryLowAction
        );
    }
    let mut capabilities = facts();
    let other = Record::authorize(
        Settings {
            enabled: true,
            actions: vec![Scope::BatteryThresholds],
            osd: false,
        },
        &capabilities,
    )
    .unwrap();
    assert!(!other.settings.actions.contains(&Scope::BatteryLowAction));
    let action = Record::authorize(
        Settings {
            enabled: true,
            actions: vec![Scope::BatteryLowAction],
            osd: false,
        },
        &capabilities,
    )
    .unwrap();
    action.verify(&capabilities).unwrap();
    capabilities
        .capabilities
        .iter_mut()
        .find(|cap| cap.scope == Scope::BatteryLowAction)
        .unwrap()
        .identity = Some("changed-plan".into());
    assert!(action.verify(&capabilities).is_err());
}
