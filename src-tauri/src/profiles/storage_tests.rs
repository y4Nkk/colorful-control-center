use super::*;
use std::{fs, fs::OpenOptions, os::windows::fs::OpenOptionsExt};
use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ;

fn settings(mode: u8) -> ProfileSettings {
    ProfileSettings {
        fan_mode: Some(mode),
        ..Default::default()
    }
}

#[test]
fn case_only_save_never_overwrites_an_existing_profile_identity() {
    let root = tempfile::tempdir().unwrap();
    save_in(root.path(), settings(0), "Gaming").unwrap();
    let before = fs::read(root.path().join("Gaming.json")).unwrap();
    let error = save_in(root.path(), settings(1), "gaming").unwrap_err();
    assert!(format!("{error:#}").contains("仅大小写不同"));
    assert_eq!(fs::read(root.path().join("Gaming.json")).unwrap(), before);
    assert_eq!(load_from(root.path(), "Gaming").unwrap().name, "Gaming");
}

#[test]
fn actual_profile_save_load_and_overwrite_keep_current_contract() {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join("档案");
    save_in(&dir, settings(0), " 均衡 ").unwrap();
    assert_eq!(load_from(&dir, "均衡").unwrap().settings.fan_mode, Some(0));
    save_in(&dir, settings(1), "均衡").unwrap();
    let saved = load_from(&dir, "均衡").unwrap();
    assert_eq!(saved.settings.fan_mode, Some(1));
    assert_eq!(saved.version, 1);
    assert_eq!(list_in(&dir).unwrap().len(), 1);
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
}

#[test]
fn profile_replace_failure_preserves_previous_file_and_error_cause() {
    let root = tempfile::tempdir().unwrap();
    save_in(root.path(), settings(0), "均衡").unwrap();
    let target = root.path().join("均衡.json");
    let original = fs::read(&target).unwrap();
    let locked = OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .open(&target)
        .unwrap();
    let error = save_in(root.path(), settings(1), "均衡").unwrap_err();
    assert!(format!("{error:#}").contains("保存档案失败"));
    assert!(
        error
            .root_cause()
            .downcast_ref::<std::io::Error>()
            .is_some()
    );
    assert_eq!(fs::read(&target).unwrap(), original);
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    drop(locked);
    save_in(root.path(), settings(1), "均衡").unwrap();
    assert_eq!(
        load_from(root.path(), "均衡").unwrap().settings.fan_mode,
        Some(1)
    );
}

#[test]
fn incomplete_temporary_files_are_ignored_and_corrupt_profiles_are_errors() {
    let root = tempfile::tempdir().unwrap();
    save_in(root.path(), settings(0), "均衡").unwrap();
    fs::write(root.path().join(".ccc-uncommitted.tmp"), b"{partial").unwrap();
    assert_eq!(list_in(root.path()).unwrap().len(), 1);
    fs::write(root.path().join("broken.json"), b"{partial").unwrap();
    assert!(
        list_in(root.path())
            .err()
            .unwrap()
            .to_string()
            .contains("损坏")
    );
    assert!(load_from(root.path(), "broken").is_err());
    assert_eq!(
        fs::read(root.path().join("broken.json")).unwrap(),
        b"{partial"
    );
}

#[test]
fn invalid_names_or_settings_never_replace_an_existing_profile() {
    let root = tempfile::tempdir().unwrap();
    save_in(root.path(), settings(0), "均衡").unwrap();
    let before = fs::read(root.path().join("均衡.json")).unwrap();
    assert!(save_in(root.path(), settings(0), "COM¹").is_err());
    assert!(save_in(root.path(), ProfileSettings::default(), "均衡").is_err());
    assert_eq!(fs::read(root.path().join("均衡.json")).unwrap(), before);
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
}
