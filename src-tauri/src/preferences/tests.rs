use super::{Controller, model::*, read, shortcut, startup};
use std::{cell::RefCell, fs};

#[test]
fn complete_contract_round_trips_and_rejects_missing_unknown_and_invalid_values() {
    let value = serde_json::to_value(Preferences::default()).unwrap();
    assert_eq!(
        serde_json::from_value::<Preferences>(value.clone()).unwrap(),
        Preferences::default()
    );
    let mut missing = value.clone();
    missing.as_object_mut().unwrap().remove("motion");
    assert!(serde_json::from_value::<Preferences>(missing).is_err());
    let mut unknown = value;
    unknown["oldTheme"] = "light".into();
    assert!(serde_json::from_value::<Preferences>(unknown).is_err());
    let mut settings = Preferences::default();
    settings.interface_scale = 101;
    assert!(settings.validate().is_err());
    settings.interface_scale = 100;
    for invalid in ["", " leading", "a\"b", "bad;css", "bad\nfont", "a\\b"] {
        settings.interface_font = invalid.into();
        assert!(settings.validate().is_err(), "{invalid}");
    }
}

#[test]
fn only_absent_files_receive_defaults_and_damaged_files_are_preserved() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("preferences.json");
    assert_eq!(read(&path).unwrap(), Preferences::default());
    let bytes = b"{broken";
    fs::write(&path, bytes).unwrap();
    let controller = Controller::new(path.clone());
    assert!(controller.snapshot().is_err());
    assert!(
        controller
            .save(Preferences::default(), |_| panic!(
                "must not apply from unknown baseline"
            ))
            .is_err()
    );
    assert_eq!(fs::read(path).unwrap(), bytes);
}

#[test]
fn saved_preferences_are_complete_persistent_and_restart_state_tracks_startup() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("preferences.json");
    let controller = Controller::new(path.clone());
    let settings = Preferences {
        theme: Theme::Dark,
        interface_font: "Segoe UI".into(),
        interface_scale: 125,
        motion: Motion::Reduce,
        close_behavior: CloseBehavior::Exit,
        always_on_top: true,
        start_minimized: true,
        startup_page: StartupPage::LastVisited,
        global_shortcut: "shift+control+KeyK".into(),
        hardware_acceleration: false,
        runtime_log_level: LogLevel::Warn,
    };
    let applied = RefCell::new(Vec::new());
    let snapshot = controller
        .save(settings.clone(), |next| {
            applied.borrow_mut().push(next.clone());
            Ok(())
        })
        .unwrap();
    assert_eq!(*applied.borrow(), vec![settings.clone()]);
    assert!(snapshot.restart_required);
    assert_eq!(read(&path).unwrap(), settings);
    let restarted = Controller::new(path).snapshot().unwrap();
    assert_eq!(restarted.settings, settings);
    assert!(!restarted.restart_required);
}

#[test]
fn failed_desktop_application_restores_previous_state_without_publishing_file() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("preferences.json");
    let controller = Controller::new(path.clone());
    let applied = RefCell::new(Vec::new());
    let next = Preferences {
        always_on_top: true,
        ..Preferences::default()
    };
    let result = controller.save(next, |settings| {
        applied.borrow_mut().push(settings.always_on_top);
        if settings.always_on_top {
            anyhow::bail!("occupied shortcut");
        }
        Ok(())
    });
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("occupied shortcut")
    );
    assert_eq!(*applied.borrow(), vec![true, false]);
    assert!(!path.exists());
    assert!(!controller.snapshot().unwrap().settings.always_on_top);
}

#[test]
fn failed_persistence_restores_desktop_and_preserves_existing_bytes() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("preferences.json");
    let original = serde_json::to_vec(&Preferences::default()).unwrap();
    fs::write(&path, &original).unwrap();
    let controller = Controller::new(path.clone());
    let handle = RefCell::new(None);
    let calls = RefCell::new(Vec::new());
    let result = controller.save(
        Preferences {
            always_on_top: true,
            ..Preferences::default()
        },
        |settings| {
            calls.borrow_mut().push(settings.always_on_top);
            if settings.always_on_top {
                *handle.borrow_mut() = Some(
                    std::fs::OpenOptions::new()
                        .read(true)
                        .share_mode(0)
                        .open(&path)
                        .unwrap(),
                );
            }
            Ok(())
        },
    );
    drop(handle);
    assert!(result.is_err());
    assert_eq!(*calls.borrow(), vec![true, false]);
    assert_eq!(fs::read(path).unwrap(), original);
}

#[test]
fn file_damage_after_startup_is_rejected_before_desktop_changes() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("preferences.json");
    let controller = Controller::new(path.clone());
    let damaged = b"{damaged after startup";
    fs::write(&path, damaged).unwrap();
    let result = controller.save(
        Preferences {
            always_on_top: true,
            ..Preferences::default()
        },
        |_| panic!("damaged preferences must not execute desktop changes"),
    );
    assert!(format!("{:#}", result.unwrap_err()).contains("软件偏好文件损坏"));
    assert_eq!(fs::read(path).unwrap(), damaged);
}

#[test]
fn external_file_changes_require_a_new_read_and_are_never_overwritten() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("preferences.json");
    let controller = Controller::new(path.clone());
    let external = Preferences {
        theme: Theme::Dark,
        ..Preferences::default()
    };
    let bytes = serde_json::to_vec(&external).unwrap();
    fs::write(&path, &bytes).unwrap();
    let result = controller.save(
        Preferences {
            always_on_top: true,
            ..Preferences::default()
        },
        |_| panic!("stale preferences must not execute desktop changes"),
    );
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("请重新读取后再保存")
    );
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert_eq!(Controller::new(path).snapshot().unwrap().settings, external);
}

#[test]
fn rollback_failure_remains_visible_with_original_saved_state() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("preferences.json");
    let controller = Controller::new(path);
    let result = controller.save(
        Preferences {
            always_on_top: true,
            ..Preferences::default()
        },
        |_| anyhow::bail!("desktop failed"),
    );
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("恢复原桌面偏好失败")
    );
    let snapshot = controller.snapshot().unwrap();
    assert!(!snapshot.settings.always_on_top);
    assert!(
        snapshot
            .desktop_error
            .unwrap()
            .contains("恢复原桌面偏好失败")
    );
}

#[test]
fn shortcut_validation_matches_recorded_physical_chords() {
    for value in [
        "",
        "control+KeyK",
        "shift+control+Space",
        "alt+Digit7",
        "super+F12",
    ] {
        assert!(shortcut::parse(value).is_ok(), "{value}");
    }
    for value in [
        "KeyK",
        "shift+KeyK",
        "Ctrl+K",
        "control+shift+KeyK",
        "control+Tab",
        "control+F13",
        " control+KeyK",
    ] {
        assert!(shortcut::parse(value).is_err(), "{value}");
    }
}

#[test]
fn acceleration_preserves_browser_defaults_custom_arguments_and_hidden_creation() {
    let config = tauri::utils::config::WindowConfig::default();
    assert!(!startup::configure(&config, true).visible);
    assert!(
        startup::configure(&config, true)
            .additional_browser_args
            .is_none()
    );
    let disabled = startup::configure(&config, false);
    let args = disabled.additional_browser_args.as_ref().unwrap();
    assert!(args.contains("msSmartScreenProtection"));
    assert!(args.ends_with("--disable-gpu"));
    let configured = startup::configure(&disabled, false);
    assert_eq!(
        configured.additional_browser_args,
        disabled.additional_browser_args
    );
    let custom = tauri::utils::config::WindowConfig {
        additional_browser_args: Some("--custom-flag".into()),
        ..config
    };
    assert_eq!(
        startup::configure(&custom, false)
            .additional_browser_args
            .unwrap(),
        "--custom-flag --disable-gpu"
    );
}

#[test]
fn every_log_threshold_maps_to_the_actual_logger_filter() {
    assert_eq!(LogLevel::Debug.filter(), log::LevelFilter::Debug);
    assert_eq!(LogLevel::Info.filter(), log::LevelFilter::Info);
    assert_eq!(LogLevel::Warn.filter(), log::LevelFilter::Warn);
    assert_eq!(LogLevel::Error.filter(), log::LevelFilter::Error);
    assert_eq!(LogLevel::Silent.filter(), log::LevelFilter::Off);
}

use std::os::windows::fs::OpenOptionsExt;

#[test]
fn interface_restart_never_replays_autostart_arguments() {
    let executable = std::path::Path::new(r"C:\isolated\control-center.exe");
    let command = super::restart::command(executable);
    assert_eq!(command.get_program(), executable);
    assert_eq!(
        command.get_args().collect::<Vec<_>>(),
        [std::ffi::OsStr::new("--restart-interface")]
    );
}

#[test]
fn interface_restart_waits_for_old_control_and_retains_the_acquired_lease() {
    use crate::controller_lease::ControllerLease;
    use std::time::Duration;
    let directory = tempfile::tempdir().unwrap();
    let name = format!(
        "Local\\CCC.preference-restart.{}.{}",
        std::process::id(),
        directory.path().file_name().unwrap().to_string_lossy()
    );
    let old = ControllerLease::try_acquire(&name).unwrap().unwrap();
    let blocked = super::restart::wait_for_lease(&name, Duration::from_millis(20));
    assert!(blocked.err().unwrap().to_string().contains("未接管硬件"));
    let release = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(60));
        drop(old);
    });
    let next = super::restart::wait_for_lease(&name, Duration::from_secs(2)).unwrap();
    assert!(ControllerLease::try_acquire(&name).unwrap().is_none());
    drop(next);
    release.join().unwrap();
    assert!(ControllerLease::try_acquire(&name).unwrap().is_some());
}

#[test]
fn shortcut_conflicts_preserve_previous_binding_and_clear_unregisters_it() {
    let previous = shortcut::parse("control+KeyK").unwrap();
    let next = shortcut::parse("alt+KeyJ").unwrap();
    let mut active = shortcut::Active {
        key: previous,
        cleanup: None,
    };
    let result = shortcut::replace(
        &mut active,
        next,
        |_| anyhow::bail!("occupied"),
        |_| panic!("must retain previous"),
    );
    assert!(result.is_err());
    assert_eq!(active.key, previous);
    let freed = RefCell::new(Vec::new());
    shortcut::replace(
        &mut active,
        None,
        |_| panic!("clearing must not register"),
        |key| {
            freed.borrow_mut().push(key);
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(*freed.borrow(), vec![previous.unwrap()]);
    assert!(active.key.is_none());
}

#[test]
fn shortcut_replacement_registers_before_release_and_tracks_failed_cleanup() {
    let previous = shortcut::parse("control+KeyK").unwrap();
    let next = shortcut::parse("alt+KeyJ").unwrap();
    let mut active = shortcut::Active {
        key: previous,
        cleanup: None,
    };
    let operations = RefCell::new(Vec::new());
    shortcut::replace(
        &mut active,
        next,
        |key| {
            operations.borrow_mut().push((true, key));
            Ok(())
        },
        |key| {
            operations.borrow_mut().push((false, key));
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(
        *operations.borrow(),
        vec![(true, next.unwrap()), (false, previous.unwrap())]
    );
    assert_eq!(active.key, next);
    let error = shortcut::replace(
        &mut active,
        previous,
        |_| Ok(()),
        |_| anyhow::bail!("release failed"),
    )
    .unwrap_err();
    assert!(error.to_string().contains("清理新快捷键失败"));
    assert_eq!(active.key, next);
    assert_eq!(active.cleanup, previous);
    shortcut::replace(
        &mut active,
        next,
        |_| panic!("no second registration"),
        |_| Ok(()),
    )
    .unwrap();
    assert_eq!(active.key, next);
    assert!(active.cleanup.is_none());
}
