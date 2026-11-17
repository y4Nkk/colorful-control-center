use super::{
    events::{STABLE_MS, Scheduler},
    model::{Binding, Record, Trigger},
    storage::Store,
};
use crate::{
    hal::power_events::{Event, Source},
    profiles::{ProfileFile, ProfileSettings},
};

#[path = "controller_tests.rs"]
mod controller_tests;
#[path = "event_tests.rs"]
mod event_tests;
#[path = "fan_fault_tests.rs"]
mod fan_fault_tests;
#[path = "test_fixture.rs"]
pub(super) mod fixture;
#[path = "ownership_tests.rs"]
mod ownership_tests;
#[path = "worker_tests.rs"]
mod worker_tests;

fn file() -> ProfileFile {
    ProfileFile {
        version: 1,
        name: "供电档案".into(),
        saved_at_ms: 1,
        settings: ProfileSettings {
            pl1_w: Some(65.0),
            ..Default::default()
        },
    }
}
fn record() -> Record {
    Record {
        enabled: true,
        ac: Some(Binding::reviewed(&file(), None).unwrap()),
        ..Default::default()
    }
}

#[test]
fn confirmed_file_digest_ignores_json_formatting_but_not_profile_edits_or_replacement() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("供电档案.json");
    let file = file();
    let binding = Binding::reviewed(&file, None).unwrap();
    std::fs::write(&path, serde_json::to_vec_pretty(&file).unwrap()).unwrap();
    assert!(binding.load(dir.path()).unwrap() == file);
    std::fs::write(&path, serde_json::to_vec(&file).unwrap()).unwrap();
    assert!(binding.load(dir.path()).unwrap() == file);
    let mut changed = file.clone();
    changed.saved_at_ms += 1;
    std::fs::write(&path, serde_json::to_vec(&changed).unwrap()).unwrap();
    assert!(binding.load(dir.path()).is_err());
    changed = file.clone();
    changed.settings.pl1_w = Some(70.0);
    std::fs::write(&path, serde_json::to_vec(&changed).unwrap()).unwrap();
    assert!(binding.load(dir.path()).is_err());
    std::fs::remove_file(path).unwrap();
    assert!(binding.load(dir.path()).is_err());
}

#[test]
fn policy_schema_rejects_unknown_fields_versions_invalid_bindings_and_enabled_empty_rules() {
    let valid = record();
    assert_eq!(
        serde_json::from_str::<Record>(&serde_json::to_string(&valid).unwrap()).unwrap(),
        valid
    );
    let mut value = serde_json::to_value(&valid).unwrap();
    value["startup"] = true.into();
    assert!(serde_json::from_value::<Record>(value).is_err());
    let mut value = serde_json::to_value(&valid).unwrap();
    value["ac"]["settings"] = serde_json::json!({});
    assert!(serde_json::from_value::<Record>(value).is_err());
    for invalid in [
        Record {
            version: 1,
            ..valid.clone()
        },
        Record {
            enabled: true,
            ..Default::default()
        },
        Record {
            ac: Some(Binding {
                profile: "../a".into(),
                ..valid.ac.clone().unwrap()
            }),
            ..valid.clone()
        },
        Record {
            ac: Some(Binding {
                digest: "0".repeat(63),
                ..valid.ac.clone().unwrap()
            }),
            ..valid.clone()
        },
        Record {
            ac: Some(Binding {
                digest: "G".repeat(64),
                ..valid.ac.clone().unwrap()
            }),
            ..valid.clone()
        },
    ] {
        assert!(invalid.validate().is_err());
    }
    assert!(Record::default().validate().is_ok());
}

#[test]
fn first_policy_restore_keeps_original_absence_and_refuses_a_second_restore() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("policy.json");
    let mut store = Store::open(path.clone()).unwrap();
    let initial = store.record();
    assert!(!store.initial_record().is_some());
    let first = record();
    store.save(&initial, &first).unwrap();
    let paused = Record {
        enabled: false,
        ..first.clone()
    };
    store.save(&first, &paused).unwrap();
    assert!(store.initial_record().is_some());
    store.restore(&paused).unwrap();
    assert!(!path.exists());
    assert_eq!(store.record(), initial);
    assert!(store.restore(&initial).is_err());
}

#[test]
fn first_policy_restore_preserves_existing_record_across_many_edits() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("policy.json");
    let initial = Record::default();
    std::fs::write(&path, serde_json::to_vec(&initial).unwrap()).unwrap();
    let mut store = Store::open(path.clone()).unwrap();
    let enabled = record();
    store.save(&initial, &enabled).unwrap();
    let paused = Record {
        enabled: false,
        ..enabled.clone()
    };
    store.save(&enabled, &paused).unwrap();
    assert!(store.restore(&enabled).is_err());
    store.restore(&paused).unwrap();
    assert_eq!(super::storage::read(&path).unwrap(), Some(initial));
}

#[test]
fn external_changes_and_corruption_are_errors_and_never_default_overwrites() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("policy.json");
    let mut store = Store::open(path.clone()).unwrap();
    let enabled = record();
    store.save(&Record::default(), &enabled).unwrap();
    let external = Record {
        enabled: false,
        ..enabled.clone()
    };
    let bytes = serde_json::to_vec(&external).unwrap();
    std::fs::write(&path, &bytes).unwrap();
    assert!(store.save(&enabled, &Record::default()).is_err());
    assert!(store.restore(&enabled).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    std::fs::write(&path, b"{broken").unwrap();
    assert!(Store::open(path.clone()).is_err());
    assert!(store.save(&enabled, &Record::default()).is_err());
    assert_eq!(std::fs::read(path).unwrap(), b"{broken");
}

#[test]
fn failed_atomic_save_preserves_previous_policy_and_restore_eligibility() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("policy.json");
    let initial = Record::default();
    let bytes = serde_json::to_vec(&initial).unwrap();
    std::fs::write(&path, &bytes).unwrap();
    let mut store = Store::open(path.clone()).unwrap();
    let mut permissions = std::fs::metadata(&path).unwrap().permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(&path, permissions).unwrap();
    let failed = store.save(&initial, &record());
    let mut permissions = std::fs::metadata(&path).unwrap().permissions();
    permissions.set_readonly(false);
    std::fs::set_permissions(&path, permissions).unwrap();
    assert!(failed.is_err());
    assert_eq!(std::fs::read(path).unwrap(), bytes);
    assert_eq!(store.record(), initial);
    assert!(!store.initial_record().is_some());
}
