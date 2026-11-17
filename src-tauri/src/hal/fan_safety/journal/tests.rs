use super::*;

fn record(path: &std::path::Path) -> serde_json::Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

#[test]
fn exit_prevents_late_compensation_and_manual_output_without_poisoning_the_next_process() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(RECORD_NAME);
    let mut journal = Journal::open(path.clone());
    journal.begin(false).unwrap();
    journal.close();
    assert!(journal.allowed().is_err());
    assert!(journal.begin(false).is_err());
    assert!(journal.begin(true).is_err());
    assert!(journal.acknowledge().is_err());
    journal.stopped().unwrap();
    Journal::open(path).allowed().unwrap();
}

#[test]
fn reading_missing_and_unclean_records_does_not_write_and_never_authorizes_a_restart() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(RECORD_NAME);
    let mut journal = Journal::open(path.clone());
    journal.allowed().unwrap();
    assert!(!path.exists());
    journal.begin(false).unwrap();
    let initial = fs::read(&path).unwrap();
    let mut restarted = Journal::open(path.clone());
    assert!(
        restarted
            .allowed()
            .unwrap_err()
            .to_string()
            .contains("因故障停止")
    );
    assert_eq!(fs::read(&path).unwrap(), initial);
    assert!(restarted.begin(false).is_err());
    assert_eq!(fs::read(&path).unwrap(), initial);
}

#[test]
fn normal_stop_allows_next_start_but_faults_survive_stop_and_require_successful_manual_confirmation()
 {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(RECORD_NAME);
    let mut journal = Journal::open(path.clone());
    journal.begin(false).unwrap();
    journal.stopped().unwrap();
    Journal::open(path.clone()).allowed().unwrap();
    journal.begin(false).unwrap();
    journal.latch().unwrap();
    journal.stopped().unwrap();
    let mut next = Journal::open(path.clone());
    assert!(next.allowed().is_err());
    assert!(next.acknowledge().is_err());
    next.begin(true).unwrap();
    assert_eq!(record(&path)["faulted"], true);
    assert!(next.allowed().is_err());
    next.acknowledge().unwrap();
    next.allowed().unwrap();
    assert_eq!(record(&path)["active"], true);
    assert!(Journal::open(path.clone()).allowed().is_err());
    next.stopped().unwrap();
    Journal::open(path).allowed().unwrap();
}

#[test]
fn invalid_or_changed_records_are_preserved_until_an_explicit_manual_start() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(RECORD_NAME);
    for bytes in [
        b"broken".as_slice(),
        br#"{"version":1,"active":false}"#,
        br#"{"version":2,"active":false,"faulted":false}"#,
        br#"{"version":1,"active":false,"faulted":false,"old":0}"#,
    ] {
        fs::write(&path, bytes).unwrap();
        let mut journal = Journal::open(path.clone());
        assert!(journal.allowed().is_err());
        assert!(journal.begin(false).is_err());
        assert_eq!(fs::read(&path).unwrap(), bytes);
        journal.begin(true).unwrap();
        assert_eq!(record(&path)["faulted"], true);
        journal.acknowledge().unwrap();
        journal.stopped().unwrap();
        journal.allowed().unwrap();
        fs::write(&path, b"outside-change").unwrap();
        assert!(journal.allowed().is_err());
        assert!(journal.latch().is_err());
        assert_eq!(fs::read(&path).unwrap(), b"outside-change");
    }
}

#[test]
fn a_failed_atomic_update_retains_the_running_marker_and_locks_the_live_session() {
    use std::os::windows::fs::OpenOptionsExt;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(RECORD_NAME);
    let mut journal = Journal::open(path.clone());
    journal.begin(false).unwrap();
    let before = fs::read(&path).unwrap();
    let held = fs::OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(&path)
        .unwrap();
    assert!(journal.stopped().is_err());
    assert_eq!(fs::read(&path).unwrap(), before);
    assert!(journal.allowed().is_err());
    drop(held);
    assert!(Journal::open(path.clone()).allowed().is_err());
    journal.begin(true).unwrap();
    journal.acknowledge().unwrap();
    journal.stopped().unwrap();
    Journal::open(path).allowed().unwrap();
}
