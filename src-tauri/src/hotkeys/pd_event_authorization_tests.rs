use super::*;
use ccc_core::oem_events::Action;

#[test]
fn pd_event_does_not_inherit_battery_or_mode_authorization() {
    assert_eq!(
        Scope::for_action(Action::PowerDeliveryEvent),
        Scope::PowerDeliveryEvent
    );
    let mut capabilities = facts();
    let settings = Settings {
        enabled: true,
        actions: vec![Scope::PowerDeliveryEvent],
        osd: true,
    };
    let record = Record::authorize(settings, &capabilities).unwrap();
    record.verify(&capabilities).unwrap();
    let target = capabilities
        .capabilities
        .iter_mut()
        .find(|cap| cap.scope == Scope::PowerDeliveryEvent)
        .unwrap();
    target.identity = Some("replacement-adapter".into());
    assert!(record.verify(&capabilities).is_err());
}
