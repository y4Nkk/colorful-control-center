use super::*;
use std::{
    fs,
    fs::OpenOptions,
    os::windows::fs::OpenOptionsExt,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Duration,
};
use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ;

#[test]
fn creates_and_replaces_complete_content_in_unicode_directory() {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join("中文 空间");
    fs::create_dir(&dir).unwrap();
    let target = dir.join("设置.json");
    write(&target, b"{\"profile\":null}").unwrap();
    write(&target, br#"{"profile":"Balanced"}"#).unwrap();
    assert_eq!(fs::read(&target).unwrap(), br#"{"profile":"Balanced"}"#);
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
}

#[test]
fn locked_target_keeps_original_and_can_be_retried_without_temp_leaks() {
    let root = tempfile::tempdir().unwrap();
    let target = root.path().join("profile.json");
    fs::write(&target, b"old").unwrap();
    let locked = OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .open(&target)
        .unwrap();
    assert!(write(&target, b"new").is_err());
    assert_eq!(fs::read(&target).unwrap(), b"old");
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    drop(locked);
    write(&target, b"new").unwrap();
    assert_eq!(fs::read(&target).unwrap(), b"new");
}

#[test]
fn abandoned_prepared_file_never_changes_original_and_is_cleaned_on_drop() {
    let root = tempfile::tempdir().unwrap();
    let target = root.path().join("profile.json");
    fs::write(&target, b"old").unwrap();
    let prepared = stage(&target, b"new").unwrap();
    assert_eq!(prepared.path().parent(), target.parent());
    assert_eq!(fs::read(&target).unwrap(), b"old");
    drop(prepared);
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
}

#[test]
fn directory_target_and_missing_parent_fail_without_changing_existing_data() {
    let root = tempfile::tempdir().unwrap();
    let target = root.path().join("occupied");
    fs::create_dir(&target).unwrap();
    fs::write(target.join("preserve"), b"old").unwrap();
    assert!(write(&target, b"new").is_err());
    assert_eq!(fs::read(target.join("preserve")).unwrap(), b"old");
    assert!(write(&root.path().join("missing/profile.json"), b"new").is_err());
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
}

#[test]
fn concurrent_readers_only_observe_complete_old_or_new_json() {
    let root = tempfile::tempdir().unwrap();
    let target = root.path().join("profile.json");
    let document = |version| {
        serde_json::to_vec(&serde_json::json!({"version":version, "data":"x".repeat(32768)}))
            .unwrap()
    };
    write(&target, &document(0)).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let reader_stop = stop.clone();
    let reader_path = target.clone();
    let (ready_send, ready) = mpsc::channel();
    let (done_send, done) = mpsc::channel();
    let reader = std::thread::spawn(move || {
        let mut reads = 0;
        while !reader_stop.load(Ordering::SeqCst) {
            let bytes = fs::read(&reader_path).unwrap();
            let value: serde_json::Value =
                serde_json::from_slice(&bytes).expect("读取到了不完整 JSON");
            assert_eq!(value["data"].as_str().unwrap().len(), 32768);
            let version = value["version"].as_u64().unwrap();
            assert!(version <= 32);
            if reads == 0 {
                assert_eq!(version, 0);
                ready_send.send(()).unwrap();
            }
            reads += 1;
            if version == 32 {
                done_send.send(()).unwrap();
                break;
            }
        }
        reads
    });
    ready.recv_timeout(Duration::from_secs(5)).unwrap();
    let writes = (1..=32).try_for_each(|version| write(&target, &document(version)));
    let complete = if writes.is_ok() {
        Some(done.recv_timeout(Duration::from_secs(5)))
    } else {
        None
    };
    stop.store(true, Ordering::SeqCst);
    let reads = reader.join().unwrap();
    writes.unwrap();
    complete.unwrap().unwrap();
    assert!(reads >= 2);
    assert_eq!(fs::read(&target).unwrap(), document(32));
}
