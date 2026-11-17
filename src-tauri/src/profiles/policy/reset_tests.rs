use super::tests::fixture::Harness;
use super::{Controller, Record};
use std::{fs, fs::OpenOptions, os::windows::fs::OpenOptionsExt};
use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ;

#[test]
fn legacy_rules_remain_unusable_and_untouched_until_exact_confirmed_reset() {
    let h = Harness::new(false);
    let path = h.dir.path().join("legacy-policy.json");
    let old = br#"{"version":1,"enabled":true,"ac":null,"battery":null,"resume":null}"#;
    fs::write(&path, old).unwrap();
    let controller = Controller::new(path.clone());
    let view = controller.read().unwrap();
    assert!(view.record.is_none() && view.configuration_error.is_some() && !view.active);
    assert_eq!(fs::read(&path).unwrap(), old);
    assert!(controller.reset(&h.f.hal, &"0".repeat(64)).is_err());
    assert_eq!(fs::read(&path).unwrap(), old);
    controller
        .reset(&h.f.hal, view.reset_revision.as_deref().unwrap())
        .unwrap();
    let view = controller.read().unwrap();
    assert_eq!(view.record.unwrap(), Record::default());
    assert!(view.paused && !view.active && view.initial.is_none());
    assert!(view.configuration_error.is_none() && view.reset_revision.is_none());
    assert!(h.f.device.0.lock().unwrap().events.is_empty());
}

#[test]
fn old_reset_cannot_overwrite_a_repaired_or_changed_file() {
    let h = Harness::new(false);
    let path = h.dir.path().join("legacy-policy.json");
    fs::write(&path, b"{legacy").unwrap();
    let controller = Controller::new(path.clone());
    let revision = controller.read().unwrap().reset_revision.unwrap();
    fs::write(&path, b"{different").unwrap();
    assert!(controller.reset(&h.f.hal, &revision).is_err());
    assert_eq!(fs::read(&path).unwrap(), b"{different");
    let repaired = serde_json::to_vec(&Record::default()).unwrap();
    fs::write(&path, &repaired).unwrap();
    assert!(controller.reset(&h.f.hal, &revision).is_err());
    assert_eq!(fs::read(&path).unwrap(), repaired);
    controller.reload().unwrap();
    assert_eq!(
        controller.read().unwrap().record.unwrap(),
        Record::default()
    );
}

#[test]
fn failed_reset_keeps_original_bytes_and_can_retry_after_the_lock_is_released() {
    let h = Harness::new(false);
    let path = h.dir.path().join("legacy-policy.json");
    let old = b"{legacy";
    fs::write(&path, old).unwrap();
    let controller = Controller::new(path.clone());
    let revision = controller.read().unwrap().reset_revision.unwrap();
    let locked = OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .open(&path)
        .unwrap();
    assert!(controller.reset(&h.f.hal, &revision).is_err());
    assert_eq!(fs::read(&path).unwrap(), old);
    drop(locked);
    controller.reset(&h.f.hal, &revision).unwrap();
    assert_eq!(
        controller.read().unwrap().record.unwrap(),
        Record::default()
    );
}
