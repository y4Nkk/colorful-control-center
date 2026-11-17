use super::*;

#[test]
fn low_battery_and_each_system_event_have_distinct_grants() {
    let facts = facts();
    for scope in [
        Scope::BatteryLowAction,
        Scope::SystemShutdown,
        Scope::LidAction,
    ] {
        let record = Record::authorize(
            Settings {
                enabled: true,
                actions: vec![scope],
                osd: false,
            },
            &facts,
        )
        .unwrap();
        for requested in [
            Scope::BatteryLowAction,
            Scope::SystemShutdown,
            Scope::LidAction,
        ] {
            assert_eq!(
                record.verify_action(&facts, requested).is_ok(),
                scope == requested
            );
        }
    }
}

#[test]
fn changed_lid_plan_rejects_its_grant_without_revoking_unrelated_shutdown() {
    let mut facts = facts();
    let record = Record::authorize(
        Settings {
            enabled: true,
            actions: vec![Scope::SystemShutdown, Scope::LidAction],
            osd: false,
        },
        &facts,
    )
    .unwrap();
    facts
        .capabilities
        .iter_mut()
        .find(|cap| cap.scope == Scope::LidAction)
        .unwrap()
        .identity = Some("changed-plan-or-modern-standby".into());
    assert!(record.verify_action(&facts, Scope::LidAction).is_err());
    record.verify_action(&facts, Scope::SystemShutdown).unwrap();
    assert_eq!(
        Scope::for_action(ccc_core::oem_events::Action::SystemShutdown),
        Scope::SystemShutdown
    );
    assert_eq!(
        Scope::for_action(ccc_core::oem_events::Action::LidAction),
        Scope::LidAction
    );
}
