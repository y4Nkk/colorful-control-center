use super::*;
use store::FileStore;

fn record() -> Record {
    Record {
        version: 1,
        baseline: model::Baseline {
            name: "原厂".into(),
            configuration: original_config(),
            flags: [0, 1],
        },
        current: original_config(),
        owned: vec![Device::Keyboard],
        phase: Phase::Prepared,
    }
}
#[test]
fn actual_atomic_store_preserves_external_changes_and_rejects_unknown_contracts() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("keymap-recovery.json");
    let store = FileStore(path.clone());
    let empty = store.load().unwrap();
    let first = store.replace(&empty, &record()).unwrap();
    assert!(first.record.is_some());
    assert!(store.replace(&empty, &record()).is_err());
    let original = std::fs::read(&path).unwrap();
    let mut invalid = record();
    invalid.version = 2;
    assert!(store.replace(&first, &invalid).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), original);
    let invalid = String::from_utf8(original)
        .unwrap()
        .replace("\"version\": 1", "\"version\": 1,\"legacy\": true");
    std::fs::write(&path, &invalid).unwrap();
    assert!(store.load().is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), invalid);
}
#[test]
fn prepared_record_can_be_reopened_for_recovery_without_initializing_any_provider() {
    let dir = tempfile::tempdir().unwrap();
    let store = FileStore(dir.path().join("recovery.json"));
    store.replace(&store.load().unwrap(), &record()).unwrap();
    let loaded = FileStore(store.0.clone()).load().unwrap();
    assert_eq!(loaded.record.unwrap().phase, Phase::Prepared);
}
