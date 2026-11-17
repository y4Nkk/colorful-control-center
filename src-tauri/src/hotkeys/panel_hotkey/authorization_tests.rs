use super::*;
#[test]
fn panel_key_needs_its_own_scope_and_selected_source_change_invalidates_authorization() {
    use ccc_core::oem_events::{Action, Packet};
    let action = Packet { code: 215, data: 0 }.action().unwrap();
    assert_eq!(action, Action::PanelHotkey);
    assert_eq!(Scope::for_action(action), Scope::PanelHotkey);
    let mut facts = facts();
    for scope in [
        Scope::Touchpad,
        Scope::ScreenBrightness,
        Scope::StateNotifications,
    ] {
        let record = Record::authorize(
            Settings {
                enabled: true,
                actions: vec![scope],
                osd: true,
            },
            &facts,
        )
        .unwrap();
        assert!(!record.settings.actions.contains(&Scope::PanelHotkey));
    }
    let record = Record::authorize(
        Settings {
            enabled: true,
            actions: vec![Scope::PanelHotkey],
            osd: true,
        },
        &facts,
    )
    .unwrap();
    facts
        .capabilities
        .iter_mut()
        .find(|c| c.scope == Scope::PanelHotkey)
        .unwrap()
        .identity = Some("new-oled-source".into());
    assert!(record.verify(&facts).is_err());
    facts
        .capabilities
        .iter_mut()
        .find(|c| c.scope == Scope::PanelHotkey)
        .unwrap()
        .identity = None;
    assert!(authorization(&record.settings, &facts).is_err());
}
