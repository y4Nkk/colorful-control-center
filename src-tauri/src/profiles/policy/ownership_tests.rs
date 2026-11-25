use super::{Record, fixture::Harness};
use crate::hal::power_events::Route;

fn mode_policy(h: &mut Harness) -> std::path::PathBuf {
    let path = h.dir.path().join("oem-supply-mode.json");
    h.f.hal.bind_automatic_mode_record(path.clone());
    let record = serde_json::json!({"version":5,"enabled":true,
        "deviceIdentity":"a".repeat(64),"identity":"b".repeat(64),
        "clockApproval":{"policyIdentity":"c".repeat(64),"driverIdentity":null},
        "captureTarget":null,"initial":null,"last":null,"attempt":null,"trackClocks":false});
    std::fs::write(&path, serde_json::to_vec(&record).unwrap()).unwrap();
    assert!(
        crate::powerctl::mode::supply::ensure_inactive(&path)
            .unwrap_err()
            .to_string()
            .contains("仍已启用")
    );
    path
}
#[test]
fn an_enabled_oem_mode_owner_blocks_new_profile_rules_but_never_blocks_disabling() {
    let mut h = Harness::new(false);
    let mode = mode_policy(&mut h);
    let request = h.request(false);
    let original = std::fs::read(&mode).unwrap();
    assert!(
        h.policy
            .save(&h.context(), &h.profiles(), &request)
            .is_err()
    );
    assert_eq!(h.record(), Record::default());
    assert_eq!(std::fs::read(&mode).unwrap(), original);
    let mut disabled = request;
    disabled.enabled = false;
    h.policy
        .save(&h.context(), &h.profiles(), &disabled)
        .unwrap();
    assert!(!h.record().enabled);
    assert!(h.f.device.0.lock().unwrap().events.is_empty());
}
#[test]
fn restoring_an_enabled_profile_baseline_rechecks_the_other_owner_before_commit() {
    let mut h = Harness::new(false);
    h.enable(false);
    h.policy =
        crate::profiles::policy::Controller::new(h.dir.path().join("power-profile-policy.json"));
    h.policy
        .shared
        .ready(Route {
            modern_standby: Some(false),
            display_events: false,
            warnings: vec![],
        })
        .unwrap();
    let mut disabled = h.request(false);
    disabled.enabled = false;
    h.policy
        .save(&h.context(), &h.profiles(), &disabled)
        .unwrap();
    assert!(h.policy.read().unwrap().initial.unwrap().enabled);
    mode_policy(&mut h);
    let before = h.record();
    assert!(h.policy.restore(&h.f.hal, &before).is_err());
    assert_eq!(h.record(), before);
    assert!(h.f.device.0.lock().unwrap().events.is_empty());
}
#[test]
fn a_corrupted_foreign_owner_file_is_preserved_and_never_interpreted_as_disabled() {
    let mut h = Harness::new(false);
    let mode = mode_policy(&mut h);
    std::fs::write(&mode, b"invalid mode policy").unwrap();
    assert!(
        h.policy
            .save(&h.context(), &h.profiles(), &h.request(false))
            .is_err()
    );
    assert_eq!(std::fs::read(&mode).unwrap(), b"invalid mode policy");
    assert!(!h.record().enabled);
}
