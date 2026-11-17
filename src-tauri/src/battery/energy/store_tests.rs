use super::*;

fn initial() -> Record {
    Record {
        version: 1,
        policy: Some(policy(false)),
        capability: capability(),
        baseline: Snapshot {
            policy: Some(policy(false)),
            charge: charge(),
            capability: capability(),
        },
        stop_charge: charge(),
        pending: None,
        last_error: None,
    }
}

#[test]
fn file_store_requires_the_exact_expected_record_before_replacement() {
    let dir = tempfile::tempdir().unwrap();
    let store = FileStore(dir.path().join("energy-policy.json"));
    let record = initial();
    store.save(&None, &record).unwrap();
    let before = std::fs::read(&store.0).unwrap();
    let mut wrong = record.clone();
    wrong.last_error = Some("外部修订".into());
    assert!(store.save(&Some(wrong.clone()), &record).is_err());
    assert_eq!(std::fs::read(&store.0).unwrap(), before);
    store.save(&Some(record), &wrong).unwrap();
    assert_eq!(store.load().unwrap(), Some(wrong));
}

#[test]
fn writer_never_publishes_a_record_larger_than_its_reader_accepts() {
    let dir = tempfile::tempdir().unwrap();
    let store = FileStore(dir.path().join("energy-policy.json"));
    let record = initial();
    store.save(&None, &record).unwrap();
    let before = std::fs::read(&store.0).unwrap();
    let mut large = record.clone();
    large.last_error = Some("x".repeat(65_536));
    assert!(store.save(&Some(record), &large).is_err());
    assert_eq!(std::fs::read(&store.0).unwrap(), before);
    assert!(store.load().unwrap().is_some());
}
