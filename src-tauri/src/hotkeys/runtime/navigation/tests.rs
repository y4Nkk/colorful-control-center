use super::*;
use crate::hotkeys::model::{Capability, Facts, Scope, Settings};
use ccc_core::oem_events::Action;

#[test]
fn navigation_has_separate_authorization_bound_to_keyboard_type() {
    assert_eq!(Scope::for_action(Action::OpenKeyboard), Scope::OpenKeyboard);
    let mut facts = Facts {
        mode_cycle: None,
        panel_hotkey: None,
        screen_brightness: None,
        gpu_turbo: None,
        battery_thresholds: None,
        pd_event: None,
        battery_low_brightness: None,
        battery_low_action: None,
        lid_action: None,
        battery_alert: None,
        capabilities: [
            Scope::OpenConsole,
            Scope::OpenKeyboard,
            Scope::DisplayRotation,
        ]
        .map(|scope| Capability {
            scope,
            identity: Some(format!("{scope:?}")),
            reason: None,
        })
        .into(),
        blocked_reason: None,
    };
    let settings = Settings {
        enabled: true,
        actions: vec![Scope::OpenConsole],
        osd: false,
    };
    let console = Record::authorize(settings, &facts).unwrap();
    assert!(!console.settings.actions.contains(&Scope::OpenKeyboard));
    let settings = Settings {
        enabled: true,
        actions: vec![Scope::OpenKeyboard],
        osd: false,
    };
    let keyboard = Record::authorize(settings, &facts).unwrap();
    keyboard.verify(&facts).unwrap();
    facts.capabilities[1].identity = Some("changed-type".into());
    assert!(keyboard.verify(&facts).is_err());
    assert_eq!(
        Scope::for_action(Action::DisplayRotation),
        Scope::DisplayRotation
    );
    assert!(!console.settings.actions.contains(&Scope::DisplayRotation));
    let display = Record::authorize(
        Settings {
            enabled: true,
            actions: vec![Scope::DisplayRotation],
            osd: false,
        },
        &facts,
    )
    .unwrap();
    display.verify(&facts).unwrap();
    facts.capabilities[2].identity = Some("replaced-primary".into());
    assert!(display.verify(&facts).is_err());
}

#[test]
fn queued_navigation_is_ready_only_after_display_and_stale_ack_cannot_remove_a_new_request() {
    let mut state = Status {
        running: true,
        ..Status::default()
    };
    let first = state.queue_navigation(NavigationKind::Keyboard).unwrap();
    assert!(state.navigation_pending(first));
    assert!(state.acknowledge_navigation(first).is_err());
    assert!(state.navigation_ready(first));
    let second = state
        .queue_navigation(NavigationKind::DisplayRotation)
        .unwrap();
    assert!(second > first);
    assert!(!state.navigation_ready(first));
    assert!(state.acknowledge_navigation(first).is_err());
    assert!(state.navigation_ready(second));
    assert_eq!(
        state.navigation_request.as_ref().unwrap().kind,
        NavigationKind::DisplayRotation
    );
    assert!(state.acknowledge_navigation(first).is_err());
    state.acknowledge_navigation(second).unwrap();
    assert!(state.navigation_request.is_none());
}

#[test]
fn clean_stop_discards_only_unshown_requests() {
    let mut state = Status {
        running: true,
        ..Status::default()
    };
    let first = state.queue_navigation(NavigationKind::Keyboard).unwrap();
    state.stop_clean();
    assert!(!state.navigation_ready(first));
    let second = state
        .queue_navigation(NavigationKind::DisplayRotation)
        .unwrap();
    assert!(state.navigation_ready(second));
    state.stop_clean();
    assert_eq!(state.navigation_request.unwrap().id, second);
}
