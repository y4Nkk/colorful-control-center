use super::*;

#[test]
fn only_exact_oem_source_requests_and_approved_status_notices_are_classified() {
    let mut event = crate::hal::event_log::Record {
        number: 1,
        event_id: 0,
        event_type: 4,
        source: "PowerBIOSServer_Out".into(),
        strings: vec!["Arg001^37".into()],
    };
    assert_eq!(
        runtime::packet(&event).unwrap(),
        Some(ccc_core::oem_events::Action::ToggleFnLock)
    );
    for text in ["clevofnkey^37", "Arg001^85", "Arg001^255"] {
        event.strings = vec![text.into()];
        assert_eq!(runtime::packet(&event).unwrap(), None);
    }
    for (text, action) in [
        ("Arg001^72", ccc_core::oem_events::Action::SystemShutdown),
        ("Arg001^192", ccc_core::oem_events::Action::LidAction),
        ("Arg001^132", ccc_core::oem_events::Action::OpenRecovery),
    ] {
        event.strings = vec![text.into()];
        assert_eq!(runtime::packet(&event).unwrap(), Some(action));
    }
    event.strings = vec!["Arg001^203".into()];
    assert_eq!(
        runtime::packet(&event).unwrap(),
        Some(ccc_core::oem_events::Action::Notify(
            ccc_core::oem_events::Notice::ScrollLock(false)
        ))
    );
    event.strings = vec!["Arg001^37".into()];
    event.source = "other".into();
    assert_eq!(runtime::packet(&event).unwrap(), None);
    event.source = "PowerBIOSServer_Out".into();
    event.strings.push("unexpected".into());
    assert!(runtime::packet(&event).is_err());
}
