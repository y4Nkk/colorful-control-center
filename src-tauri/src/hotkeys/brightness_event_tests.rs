use super::*;

#[test]
fn brightness_event_has_its_own_permission_and_does_not_inherit_up_down_switch_or_notices() {
    use ccc_core::oem_events::{Action, Packet};
    assert_eq!(
        Packet { code: 138, data: 0 }.action(),
        Some(Action::KeyboardBrightnessEvent)
    );
    assert_eq!(
        Scope::for_action(Action::KeyboardBrightnessEvent),
        Scope::KeyboardBrightnessEvent
    );
    let facts = facts();
    for scope in [
        Scope::KeyboardBrightness,
        Scope::KeyboardSwitch,
        Scope::StateNotifications,
    ] {
        let old = Record::authorize(
            Settings {
                enabled: true,
                actions: vec![scope],
                osd: true,
            },
            &facts,
        )
        .unwrap();
        old.verify(&facts).unwrap();
        assert!(
            !old.settings
                .actions
                .contains(&Scope::KeyboardBrightnessEvent)
        );
    }
    let saved = Record::authorize(
        Settings {
            enabled: true,
            actions: vec![Scope::KeyboardBrightnessEvent],
            osd: true,
        },
        &facts,
    )
    .unwrap();
    saved.verify(&facts).unwrap();
    let mut changed = facts.clone();
    changed
        .capabilities
        .iter_mut()
        .find(|c| c.scope == Scope::KeyboardBrightnessEvent)
        .unwrap()
        .identity = Some("new-rules".into());
    assert!(saved.verify(&changed).is_err());
}

#[test]
fn brightness_completion_separates_cache_update_status_from_vendor_desktop_style() {
    use crate::lighting::brightness_event::Report;
    let quiet = crate::hotkeys::actions::keyboard_actions::brightness_completion(Some(Report {
        level: 3,
        display_maximum: 2,
        firmware_report: true,
    }));
    assert!(!quiet.show_osd);
    let message = quiet.message.unwrap();
    assert!(message.detail.contains("第 3 档"));
    assert!(message.level.is_none());
    assert!(!message.error);
    for (level, maximum, firmware_report) in [(0, 2, true), (5, 5, true), (0, 4, false)] {
        let result =
            crate::hotkeys::actions::keyboard_actions::brightness_completion(Some(Report {
                level,
                display_maximum: maximum,
                firmware_report,
            }));
        assert!(result.show_osd);
        let message = result.message.unwrap();
        assert_eq!(message.level.as_ref().unwrap().value, level);
        assert_eq!(message.level.unwrap().max, maximum);
    }
    let noop = crate::hotkeys::actions::keyboard_actions::brightness_completion(None);
    assert!(noop.message.is_none());
    assert!(!noop.show_osd);
}

#[test]
fn color_cycle_requires_its_own_palette_bound_authorization() {
    let scope = Scope::for_action(ccc_core::oem_events::Action::CycleKeyboardColor);
    assert_eq!(scope, Scope::KeyboardColor);
    let settings = Settings {
        enabled: true,
        actions: vec![
            Scope::KeyboardBrightness,
            Scope::KeyboardSwitch,
            Scope::KeyboardZones,
        ],
        osd: true,
    };
    assert!(!settings.actions.contains(&scope));
    let settings = Settings {
        actions: vec![scope],
        ..settings
    };
    let mut facts = facts();
    let original = authorization(&settings, &facts).unwrap();
    facts
        .capabilities
        .iter_mut()
        .find(|c| c.scope == scope)
        .unwrap()
        .identity = Some("new-palette".into());
    assert_ne!(authorization(&settings, &facts).unwrap(), original);
    facts
        .capabilities
        .iter_mut()
        .find(|c| c.scope == scope)
        .unwrap()
        .identity = None;
    assert!(authorization(&settings, &facts).is_err());
}
