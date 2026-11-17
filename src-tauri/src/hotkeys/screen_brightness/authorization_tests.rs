use super::*;
use ccc_core::{
    display_brightness::EVENTS,
    oem_events::{Action, Packet},
};

#[test]
fn all_eleven_screen_events_need_their_own_authorization_and_never_inherit_keyboard_scopes() {
    let facts = facts();
    for code in EVENTS {
        for data in [-1, 0, 127] {
            let action = Packet { code, data }.action().unwrap();
            assert_eq!(action, Action::DisplayBrightness(code));
            assert_eq!(Scope::for_action(action), Scope::ScreenBrightness);
        }
    }
    for old_scope in [
        Scope::KeyboardBrightness,
        Scope::KeyboardBrightnessEvent,
        Scope::StateNotifications,
    ] {
        let record = Record::authorize(
            Settings {
                enabled: true,
                actions: vec![old_scope],
                osd: true,
            },
            &facts,
        )
        .unwrap();
        record.verify(&facts).unwrap();
        assert!(!record.settings.actions.contains(&Scope::ScreenBrightness));
    }
    let record = Record::authorize(
        Settings {
            enabled: true,
            actions: vec![Scope::ScreenBrightness],
            osd: false,
        },
        &facts,
    )
    .unwrap();
    let mut changed = facts;
    changed
        .capabilities
        .iter_mut()
        .find(|c| c.scope == Scope::ScreenBrightness)
        .unwrap()
        .identity = Some("changed-panel-plan-targets".into());
    assert!(record.verify(&changed).is_err());
}
