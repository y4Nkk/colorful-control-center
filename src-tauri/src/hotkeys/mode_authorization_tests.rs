use super::*;
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

struct PrivateFile(PathBuf);
impl PrivateFile {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        Self(std::env::temp_dir().join(format!(
                "ccc-mode-hotkey-{}-{}-{}.json",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                NEXT.fetch_add(1, Ordering::SeqCst)
            )))
    }
}
impl Drop for PrivateFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
fn saved(path: &PrivateFile) -> Record {
    let loaded = store::load(&path.0).unwrap();
    let approved = Record::authorize(enabled(), &facts()).unwrap();
    store::replace(&path.0, &loaded, &approved).unwrap().record
}

#[test]
fn old_or_missing_mode_driver_record_is_rejected_without_rewriting_private_file() {
    let path = PrivateFile::new();
    let current = Record::authorize(enabled(), &facts()).unwrap();
    for change in ["missing", "old-version", "unknown-field"] {
        let mut value = serde_json::to_value(&current).unwrap();
        match change {
            "missing" => {
                value.as_object_mut().unwrap().remove("modeDriver");
            }
            "old-version" => value["version"] = 1.into(),
            _ => value["legacyDriver"] = serde_json::Value::Null,
        }
        let bytes = serde_json::to_vec(&value).unwrap();
        std::fs::write(&path.0, &bytes).unwrap();
        assert!(store::load(&path.0).is_err());
        assert_eq!(std::fs::read(&path.0).unwrap(), bytes);
    }
}

#[test]
fn driver_enrollment_persists_once_updates_the_running_record_and_rejects_replacement() {
    let path = PrivateFile::new();
    let mut approved = saved(&path);
    let identity = "a".repeat(64);
    store::bind_mode_driver(&path.0, &mut approved, &identity).unwrap();
    let loaded = store::load(&path.0).unwrap();
    assert_eq!(approved.mode_driver.as_deref(), Some(identity.as_str()));
    assert_eq!(loaded.record, approved);
    store::bind_mode_driver(&path.0, &mut approved, &identity).unwrap();
    assert_eq!(store::load(&path.0).unwrap().revision, loaded.revision);
    assert!(store::bind_mode_driver(&path.0, &mut approved, &"b".repeat(64)).is_err());
    assert_eq!(store::load(&path.0).unwrap().record, loaded.record);
    assert_eq!(approved, loaded.record);
}

#[test]
fn revoked_or_corrupt_permission_never_enrolls_a_new_gpu_driver() {
    let path = PrivateFile::new();
    let mut approved = saved(&path);
    assert!(store::bind_mode_driver(&path.0, &mut approved, "bad-identity").is_err());
    assert!(approved.mode_driver.is_none());
    let loaded = store::load(&path.0).unwrap();
    store::replace(&path.0, &loaded, &Record::default()).unwrap();
    let bytes = std::fs::read(&path.0).unwrap();
    assert!(store::bind_mode_driver(&path.0, &mut approved, &"a".repeat(64)).is_err());
    assert!(approved.mode_driver.is_none());
    assert_eq!(std::fs::read(&path.0).unwrap(), bytes);
}

#[test]
fn non_mode_and_disabled_permissions_cannot_contain_or_enroll_mode_driver() {
    let path = PrivateFile::new();
    let mut approved = saved(&path);
    approved.settings.actions = vec![Scope::FnLock];
    assert!(store::bind_mode_driver(&path.0, &mut approved, &"a".repeat(64)).is_err());
    approved.mode_driver = Some("a".repeat(64));
    assert!(approved.validate().is_err());
    approved = Record::default();
    approved.mode_driver = Some("a".repeat(64));
    assert!(approved.validate().is_err());
}

#[test]
fn runtime_action_checks_only_its_grant_and_never_inherits_another_scope() {
    let record = Record::authorize(enabled(), &facts()).unwrap();
    let mut one = facts();
    one.capabilities.retain(|cap| cap.scope == Scope::PowerMode);
    record.verify_action(&one, Scope::PowerMode).unwrap();
    assert!(record.verify(&one).is_err());
    assert!(record.verify_action(&one, Scope::FnLock).is_err());
    assert!(record.verify_action(&one, Scope::GpuTurbo).is_err());
    one.capabilities[0].identity = Some("changed-mode".into());
    assert!(record.verify_action(&one, Scope::PowerMode).is_err());
}

#[test]
fn ordered_grants_are_the_only_authority_and_reject_duplicate_or_missing_scopes() {
    let record = Record::authorize(enabled(), &facts()).unwrap();
    for change in ["duplicate", "missing", "unsorted", "empty"] {
        let mut changed = record.clone();
        let grants = changed.authorization.as_mut().unwrap();
        match change {
            "duplicate" => grants.push(grants[0].clone()),
            "missing" => {
                grants.pop();
            }
            "unsorted" => grants.reverse(),
            _ => grants[0].1.clear(),
        }
        assert!(changed.validate().is_err());
    }
}

#[test]
fn invalid_file_reset_requires_exact_raw_revision_and_leaves_valid_or_changed_file_untouched() {
    let path = PrivateFile::new();
    let old = br#"{"version":1,"settings":{"enabled":true,"actions":["powerMode"],"osd":true},"authorization":"old"}"#;
    std::fs::write(&path.0, old).unwrap();
    let inspected = store::inspect(&path.0).unwrap();
    assert!(inspected.loaded.is_none() && inspected.error.is_some());
    let revision = inspected.reset_revision.unwrap();
    assert_eq!(std::fs::read(&path.0).unwrap(), old);
    std::fs::write(&path.0, b"changed-invalid-record").unwrap();
    assert!(store::reset(&path.0, &revision).is_err());
    assert_eq!(std::fs::read(&path.0).unwrap(), b"changed-invalid-record");
    let revision = store::inspect(&path.0).unwrap().reset_revision.unwrap();
    store::reset(&path.0, &revision).unwrap();
    assert_eq!(store::load(&path.0).unwrap().record, Record::default());
    let bytes = std::fs::read(&path.0).unwrap();
    assert!(store::reset(&path.0, &revision).is_err());
    assert_eq!(std::fs::read(&path.0).unwrap(), bytes);
}
