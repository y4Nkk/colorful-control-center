use super::*;

fn stored(name: &str) -> Binding {
    Binding {
        profile: name.into(),
        digest: "a".repeat(64),
        mode: None,
    }
}

#[test]
fn startup_application_message_tracks_pending_success_and_failure() {
    let mut result = crate::profiles::Application {
        id: 1,
        revision: 1,
        origin: crate::profiles::application::Origin::Startup,
        targets_applied: false,
        name: "显示档案".into(),
        steps: vec![],
        pending: Some(crate::display::Pending {
            before: vec![],
            rotation: false,
            automatic: false,
            trial_id: Some(1),
            targets: vec![],
            remaining_ms: Some(1000),
            profile_session: Some(1),
            error: None,
        }),
    };
    assert!(describe_application(&result).contains("等待显示"));
    assert!(!describe_application(&result).contains("已完成"));
    result.pending.as_mut().unwrap().error = Some("隔离恢复失败".into());
    assert!(describe_application(&result).contains("显示恢复待处理"));
    result.pending.as_mut().unwrap().error = None;
    result.pending = None;
    result.targets_applied = true;
    assert!(describe_application(&result).contains("已完成"));
    result.steps.push(crate::profiles::ApplyStep {
        step: "网络授权收尾".into(),
        ok: false,
        message: "隔离失败".into(),
    });
    assert!(describe_application(&result).contains("网络授权收尾：隔离失败"));
    assert!(describe_application(&result).contains("目标设置已应用，收尾需处理"));
    result.targets_applied = false;
    result.steps[0].step = "GPU".into();
    assert!(describe_application(&result).contains("GPU：隔离失败"));
    assert!(!describe_application(&result).contains("已完成"));
}

use std::{fs, fs::OpenOptions, os::windows::fs::OpenOptionsExt};
use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ;

#[test]
fn deleting_selected_profile_clears_its_startup_selection() {
    let root = tempfile::tempdir().unwrap();
    write_to(
        root.path(),
        &Preferences {
            profile: Some(stored("游戏")),
            ..Preferences::default()
        },
    )
    .unwrap();
    let file = root.path().join("游戏.json");
    fs::write(&file, "profile").unwrap();
    assert!(
        unlink_from(root.path(), "游戏", || fs::remove_file(&file)
            .map_err(Into::into))
        .unwrap()
    );
    assert!(!file.exists());
    assert!(read_from(root.path()).unwrap().profile.is_none());
}

#[test]
fn deletion_uses_windows_name_identity_for_existing_startup_reference() {
    let root = tempfile::tempdir().unwrap();
    write_to(
        root.path(),
        &Preferences {
            profile: Some(stored("Gaming")),
            ..Preferences::default()
        },
    )
    .unwrap();
    assert!(unlink_from(root.path(), "gaming", || Ok(())).unwrap());
    assert!(read_from(root.path()).unwrap().profile.is_none());
}
#[test]
fn failed_profile_deletion_restores_original_startup_choice() {
    let root = tempfile::tempdir().unwrap();
    write_to(
        root.path(),
        &Preferences {
            profile: Some(stored("游戏")),
            ..Preferences::default()
        },
    )
    .unwrap();
    let before = fs::read(root.path().join("startup-profile.json")).unwrap();
    let error =
        unlink_from(root.path(), "游戏", || Err(anyhow!("删除被占用档案失败"))).unwrap_err();
    assert!(format!("{error:#}").contains("启动选择保持原状态"));
    assert_eq!(
        fs::read(root.path().join("startup-profile.json")).unwrap(),
        before
    );
}
#[test]
fn deleting_another_profile_preserves_startup_file_exactly() {
    let root = tempfile::tempdir().unwrap();
    write_to(
        root.path(),
        &Preferences {
            profile: Some(stored("保留")),
            ..Preferences::default()
        },
    )
    .unwrap();
    let path = root.path().join("startup-profile.json");
    let before = fs::read(&path).unwrap();
    assert!(!unlink_from(root.path(), "其他", || Ok(())).unwrap());
    assert_eq!(fs::read(path).unwrap(), before);
}
#[test]
fn failed_selection_unlink_does_not_attempt_profile_deletion() {
    let root = tempfile::tempdir().unwrap();
    write_to(
        root.path(),
        &Preferences {
            profile: Some(stored("游戏")),
            ..Preferences::default()
        },
    )
    .unwrap();
    let locked = OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .open(root.path().join("startup-profile.json"))
        .unwrap();
    let attempted = std::cell::Cell::new(false);
    assert!(
        unlink_from(root.path(), "游戏", || {
            attempted.set(true);
            Ok(())
        })
        .is_err()
    );
    assert!(!attempted.get());
    drop(locked);
    assert_eq!(
        read_from(root.path())
            .unwrap()
            .profile
            .as_ref()
            .map(|binding| binding.profile.as_str()),
        Some("游戏")
    );
}
#[test]
fn deletion_and_choice_restore_failure_reports_safe_unlinked_state() {
    let root = tempfile::tempdir().unwrap();
    write_to(
        root.path(),
        &Preferences {
            profile: Some(stored("游戏")),
            ..Preferences::default()
        },
    )
    .unwrap();
    let lock = std::cell::RefCell::new(None);
    let error = unlink_from(root.path(), "游戏", || {
        *lock.borrow_mut() = Some(
            OpenOptions::new()
                .read(true)
                .share_mode(FILE_SHARE_READ)
                .open(root.path().join("startup-profile.json"))?,
        );
        Err(anyhow!("删除失败"))
    })
    .unwrap_err();
    assert!(format!("{error:#}").contains("当前已取消开机应用"));
    assert!(read_from(root.path()).unwrap().profile.is_none());
    drop(lock);
}

#[test]
fn startup_selection_roundtrips_and_can_be_cleared() {
    let root = tempfile::tempdir().unwrap();
    assert!(read_from(root.path()).unwrap().profile.is_none());
    write_to(
        root.path(),
        &Preferences {
            profile: Some(stored("均衡")),
            ..Preferences::default()
        },
    )
    .unwrap();
    assert_eq!(
        read_from(root.path())
            .unwrap()
            .profile
            .as_ref()
            .map(|binding| binding.profile.as_str()),
        Some("均衡")
    );
    write_to(root.path(), &Preferences::default()).unwrap();
    assert!(read_from(root.path()).unwrap().profile.is_none());
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
}

#[test]
fn failed_startup_replace_preserves_previous_selection_and_can_be_retried() {
    let root = tempfile::tempdir().unwrap();
    write_to(
        root.path(),
        &Preferences {
            profile: Some(stored("均衡")),
            ..Preferences::default()
        },
    )
    .unwrap();
    let path = root.path().join("startup-profile.json");
    let before = fs::read(&path).unwrap();
    let locked = OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .open(&path)
        .unwrap();
    let error = write_to(root.path(), &Preferences::default()).unwrap_err();
    assert!(format!("{error:#}").contains("保存启动档案失败"));
    assert_eq!(fs::read(&path).unwrap(), before);
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    drop(locked);
    write_to(root.path(), &Preferences::default()).unwrap();
    assert!(read_from(root.path()).unwrap().profile.is_none());
}

#[test]
fn corrupt_or_unknown_startup_data_is_never_silently_reset() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("startup-profile.json");
    for data in [
        "{partial",
        "{\"obsolete\":true}",
        r#"{"profile":"均衡"}"#,
        r#"{"version":1,"profile":"均衡"}"#,
        r#"{"version":2,"profile":"均衡"}"#,
        r#"{"version":5,"profile":null}"#,
        r#"{"version":3,"profile":null}"#,
        r#"{"version":4}"#,
        r#"{"version":3,"profile":null,"obsolete":true}"#,
    ] {
        fs::write(&path, data).unwrap();
        assert!(
            read_from(root.path())
                .err()
                .unwrap()
                .to_string()
                .contains("重新预览并保存启动选择")
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), data);
    }
}

#[test]
fn only_an_explicit_current_selection_replaces_an_old_record() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("startup-profile.json");
    let old = br#"{"profile":"old"}"#;
    fs::write(&path, old).unwrap();
    assert!(read_from(root.path()).is_err());
    assert_eq!(fs::read(&path).unwrap(), old);
    write_to(root.path(), &Preferences::selected(Some(stored("已确认")))).unwrap();
    let current: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(
        current,
        serde_json::json!({"version":4,"profile":{"profile":"已确认","digest":"a".repeat(64),"mode":null}})
    );
    assert_eq!(
        read_from(root.path())
            .unwrap()
            .profile
            .as_ref()
            .map(|binding| binding.profile.as_str()),
        Some("已确认")
    );
    write_to(root.path(), &Preferences::default()).unwrap();
    assert!(read_from(root.path()).unwrap().profile.is_none());
}

#[test]
fn writer_refuses_obsolete_authorization_before_creating_or_replacing_data() {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join("preferences");
    let obsolete = Preferences {
        version: 1,
        profile: Some(stored("旧授权")),
    };
    assert!(write_to(&dir, &obsolete).is_err());
    assert!(!dir.exists());
    write_to(&dir, &Preferences::default()).unwrap();
    let path = dir.join("startup-profile.json");
    let before = fs::read(&path).unwrap();
    assert!(write_to(&dir, &obsolete).is_err());
    assert_eq!(fs::read(&path).unwrap(), before);
}

#[test]
fn startup_selection_binds_reviewed_device_settings_and_rejects_changed_file() {
    let reviewed = crate::profiles::ProfileFile {
        version: 1,
        name: "完整设备".into(),
        saved_at_ms: 1,
        settings: crate::profiles::ProfileSettings {
            dust_schedule: Some(crate::fanctl::dust::Schedule {
                weekdays: 1,
                hour: 9,
                on_resume: false,
            }),
            ..Default::default()
        },
    };
    let mut changed = reviewed.clone();
    changed.settings.dust_schedule.as_mut().unwrap().hour = 10;
    assert!(
        reviewed_selection(Some(reviewed.clone()), None, |_| Ok(changed))
            .unwrap_err()
            .to_string()
            .contains("已变化")
    );
    let expected = Binding::reviewed(&reviewed, None).unwrap();
    assert_eq!(
        reviewed_selection(Some(reviewed.clone()), None, |_| Ok(reviewed)).unwrap(),
        Some(expected)
    );
    assert_eq!(
        reviewed_selection(None, None, |_| panic!("取消选择不得读取档案")).unwrap(),
        None
    );
}
