use super::{
    model::*,
    runtime, store,
    transaction::{self, Io},
};
use anyhow::{Result, bail};
use std::cell::{Cell, RefCell};

fn facts() -> Facts {
    Facts {
        mode_cycle: None,
        panel_hotkey: None,
        screen_brightness: None,
        battery_low_brightness: None,
        battery_low_action: None,
        lid_action: None,
        battery_alert: None,
        gpu_turbo: None,
        battery_thresholds: None,
        pd_event: None,
        capabilities: SCOPES
            .iter()
            .map(|scope| Capability {
                scope: *scope,
                identity: Some(format!("device-{scope:?}")),
                reason: None,
            })
            .collect(),
        blocked_reason: None,
    }
}
fn enabled() -> Settings {
    Settings {
        enabled: true,
        actions: vec![Scope::PowerMode, Scope::FnLock],
        osd: true,
    }
}
#[test]
fn authorization_binds_selected_capabilities_but_not_transient_values_or_unselected_devices() {
    let mut capabilities = facts();
    let record = Record::authorize(enabled(), &capabilities).unwrap();
    record.verify(&capabilities).unwrap();
    capabilities.capabilities[2].identity = Some("unselected-new-keyboard".into());
    record.verify(&capabilities).unwrap();
    capabilities.capabilities[0].identity = Some("new-mode-capability".into());
    assert!(record.verify(&capabilities).is_err());
    capabilities.blocked_reason = Some("原厂进程存在".into());
    assert!(
        record
            .verify(&capabilities)
            .unwrap_err()
            .to_string()
            .contains("原厂")
    );
}
#[test]
fn disabled_policy_needs_no_available_hardware_and_invalid_scopes_are_rejected() {
    let unavailable = Facts {
        mode_cycle: None,
        panel_hotkey: None,
        screen_brightness: None,
        battery_low_brightness: None,
        battery_low_action: None,
        lid_action: None,
        battery_alert: None,
        gpu_turbo: None,
        battery_thresholds: None,
        pd_event: None,
        capabilities: vec![],
        blocked_reason: Some("原厂".into()),
    };
    Record::authorize(Settings::default(), &unavailable)
        .unwrap()
        .validate()
        .unwrap();
    assert!(Record::authorize(enabled(), &unavailable).is_err());
    let mut settings = enabled();
    settings.actions.push(Scope::FnLock);
    assert!(settings.validate().is_err());
    settings.actions = vec![Scope::FnLock, Scope::PowerMode];
    assert!(settings.validate().is_err());
    settings.actions.clear();
    assert!(settings.validate().is_err());
}

#[test]
fn gpu_turbo_requires_its_own_authorization_and_source_identity() {
    use ccc_core::oem_events::Action;
    assert_eq!(Scope::for_action(Action::GpuTurbo(96)), Scope::GpuTurbo);
    assert_eq!(Scope::for_action(Action::GpuTurbo(97)), Scope::GpuTurbo);
    let mut facts = facts();
    let settings = Settings {
        enabled: true,
        actions: vec![Scope::GpuTurbo],
        osd: true,
    };
    let record = Record::authorize(settings, &facts).unwrap();
    record.verify(&facts).unwrap();
    facts
        .capabilities
        .iter_mut()
        .find(|cap| cap.scope == Scope::GpuTurbo)
        .unwrap()
        .identity = Some("replacement-gpu".into());
    assert!(record.verify(&facts).is_err());
    let capability = facts
        .capabilities
        .iter_mut()
        .find(|cap| cap.scope == Scope::GpuTurbo)
        .unwrap();
    capability.identity = None;
    capability.reason = Some("GPU 锁频不可读".into());
    assert!(Record::authorize(record.settings.clone(), &facts).is_err());
}
struct Memory {
    loaded: RefCell<store::Loaded>,
    facts: RefCell<Facts>,
    running: Cell<bool>,
    prepare_fail: Cell<bool>,
    activate_fail: Cell<bool>,
    save_fail: Cell<bool>,
    saves: Cell<usize>,
    save_attempts: Cell<usize>,
    fail_save_at: Cell<usize>,
}
impl Memory {
    fn new(running: bool) -> Self {
        let record = if running {
            Record::authorize(enabled(), &facts()).unwrap()
        } else {
            Record::default()
        };
        Self {
            loaded: RefCell::new(store::Loaded {
                record,
                revision: "initial".into(),
            }),
            facts: RefCell::new(facts()),
            running: Cell::new(running),
            prepare_fail: Cell::new(false),
            activate_fail: Cell::new(false),
            save_fail: Cell::new(false),
            saves: Cell::new(0),
            save_attempts: Cell::new(0),
            fail_save_at: Cell::new(0),
        }
    }
    fn revision(&self) -> String {
        transaction::revision(&self.loaded.borrow(), &self.facts.borrow()).unwrap()
    }
    fn restore_revision(&self, initial: &Record) -> String {
        transaction::restore_revision(&self.loaded.borrow(), initial).unwrap()
    }
}
impl Io for Memory {
    type Prepared = Record;
    fn load(&self) -> Result<store::Loaded> {
        Ok(self.loaded.borrow().clone())
    }
    fn facts(&self) -> Facts {
        self.facts.borrow().clone()
    }
    fn stop(&self) -> Result<bool> {
        Ok(self.running.replace(false))
    }
    fn prepare(&self, record: &Record) -> Result<Record> {
        if self.prepare_fail.replace(false) {
            bail!("准备订阅失败");
        }
        record.verify(&self.facts())?;
        Ok(record.clone())
    }
    fn activate(&self, _: Record) -> Result<()> {
        if self.activate_fail.replace(false) {
            bail!("激活失败");
        }
        self.running.set(true);
        Ok(())
    }
    fn save(&self, expected: &store::Loaded, target: &Record) -> Result<store::Loaded> {
        self.save_attempts.set(self.save_attempts.get() + 1);
        if self.save_fail.replace(false) || self.fail_save_at.get() == self.save_attempts.get() {
            bail!("原子提交失败");
        }
        assert_eq!(expected.revision, self.loaded.borrow().revision);
        let revision = format!("revision-{}", self.saves.get() + 1);
        self.saves.set(self.saves.get() + 1);
        let loaded = store::Loaded {
            record: target.clone(),
            revision,
        };
        *self.loaded.borrow_mut() = loaded.clone();
        Ok(loaded)
    }
}
#[test]
fn successful_apply_captures_initial_policy_and_disabled_target_stops_runtime() {
    let io = Memory::new(false);
    let mut initial = None;
    transaction::change(
        &io,
        &io.revision(),
        |_, facts| Record::authorize(enabled(), facts),
        &mut initial,
    )
    .unwrap();
    assert!(io.running.get());
    assert_eq!(initial, Some(Record::default()));
    let revision = io.restore_revision(initial.as_ref().unwrap());
    transaction::restore(&io, &revision, &mut initial).unwrap();
    assert!(!io.running.get());
    assert_eq!(initial, Some(Record::default()));
}
#[test]
fn prepare_save_and_activation_failures_restore_old_policy_and_listener() {
    for failure in 0..3 {
        let io = Memory::new(true);
        let original = io.loaded.borrow().record.clone();
        let mut initial = None;
        match failure {
            0 => io.prepare_fail.set(true),
            1 => io.save_fail.set(true),
            _ => io.activate_fail.set(true),
        }
        let mut settings = enabled();
        settings.osd = false;
        assert!(
            transaction::change(
                &io,
                &io.revision(),
                |_, facts| Record::authorize(settings, facts),
                &mut initial
            )
            .is_err()
        );
        assert_eq!(io.loaded.borrow().record, original);
        assert!(io.running.get());
        assert_eq!(initial, Some(original));
    }
}
#[test]
fn failed_compensation_keeps_initial_policy_for_explicit_retry() {
    let io = Memory::new(false);
    let mut initial = None;
    io.activate_fail.set(true);
    io.fail_save_at.set(2);
    let error = transaction::change(
        &io,
        &io.revision(),
        |_, facts| Record::authorize(enabled(), facts),
        &mut initial,
    )
    .unwrap_err();
    assert!(error.to_string().contains("恢复原策略"));
    assert_eq!(initial, Some(Record::default()));
    assert!(!io.running.get());
    assert!(io.loaded.borrow().record.settings.enabled);
    let revision = io.restore_revision(initial.as_ref().unwrap());
    transaction::restore(&io, &revision, &mut initial).unwrap();
    assert_eq!(io.loaded.borrow().record, Record::default());
}
#[test]
fn stale_preview_and_changed_initial_authorization_do_not_stop_existing_listener() {
    let io = Memory::new(true);
    let revision = io.revision();
    let initial = io.loaded.borrow().record.clone();
    let restore_revision = io.restore_revision(&initial);
    io.facts.borrow_mut().capabilities[0].identity = Some("new".into());
    assert!(transaction::change(&io, &revision, |_, _| Ok(Record::default()), &mut None).is_err());
    assert!(io.running.get());
    assert_eq!(io.saves.get(), 0);
    assert!(transaction::restore(&io, &restore_revision, &mut Some(initial)).is_err());
    assert!(io.running.get());
    assert_eq!(io.saves.get(), 0);
}
#[test]
fn software_restore_revision_survives_hardware_changes_but_rejects_new_policy() {
    let io = Memory::new(true);
    let mut initial = Some(Record::default());
    let expected = io.restore_revision(initial.as_ref().unwrap());
    let apply_revision = io.revision();
    io.facts.borrow_mut().capabilities[0].identity = Some("changed-device".into());
    assert_ne!(apply_revision, io.revision());
    assert!(transaction::restore(&io, &apply_revision, &mut initial).is_err());
    assert!(io.running.get());
    transaction::restore(&io, &expected, &mut initial).unwrap();
    assert_eq!(io.loaded.borrow().record, Record::default());
    assert!(!io.running.get());
    assert!(transaction::restore(&io, &expected, &mut initial).is_err());
    assert_eq!(io.saves.get(), 1);
}
#[test]
fn strict_store_preserves_damaged_and_externally_changed_files() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("policy.json");
    let missing = store::load(&path).unwrap();
    assert_eq!(missing.record, Record::default());
    let saved = store::replace(&path, &missing, &Record::default()).unwrap();
    std::fs::write(&path, b"{broken").unwrap();
    assert!(store::load(&path).is_err());
    assert!(store::replace(&path, &saved, &Record::default()).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"{broken");
    for invalid in [
        r#"{"version":2,"settings":{"enabled":false,"actions":[],"osd":true},"authorization":null}"#,
        r#"{"version":1,"settings":{"enabled":false,"actions":[],"osd":true},"authorization":null,"old":1}"#,
    ] {
        std::fs::write(&path, invalid).unwrap();
        assert!(store::load(&path).is_err());
    }
}
#[test]
fn state_notifications_are_separately_authorized_and_never_inherit_write_scopes() {
    use ccc_core::oem_events::{Action, Notice};
    assert_eq!(
        Scope::for_action(Action::Notify(Notice::CapsLock(true))),
        Scope::StateNotifications
    );
    let facts = facts();
    let old = Record::authorize(enabled(), &facts).unwrap();
    old.verify(&facts).unwrap();
    assert!(!old.settings.actions.contains(&Scope::StateNotifications));
    let settings = Settings {
        enabled: true,
        actions: vec![Scope::StateNotifications],
        osd: false,
    };
    let saved = Record::authorize(settings, &facts).unwrap();
    saved.verify(&facts).unwrap();
    assert_eq!(saved.settings.actions, [Scope::StateNotifications]);
    let decoded: Record = serde_json::from_slice(&serde_json::to_vec(&saved).unwrap()).unwrap();
    decoded.verify(&facts).unwrap();
    let mut changed = facts.clone();
    changed
        .capabilities
        .iter_mut()
        .find(|c| c.scope == Scope::StateNotifications)
        .unwrap()
        .identity = Some("changed".into());
    assert!(saved.verify(&changed).is_err());
    let mut encoded = serde_json::to_value(&saved).unwrap();
    encoded["settings"]["actions"] = serde_json::json!(["notifications"]);
    assert!(serde_json::from_value::<Record>(encoded).is_err());
}

#[test]
fn keyboard_switch_requires_separate_explicit_scope_and_its_device_binding() {
    let action = ccc_core::oem_events::Action::ToggleKeyboard { rgb_only: false };
    let scope = Scope::for_action(action);
    assert_eq!(scope, Scope::KeyboardSwitch);
    let mut settings = Settings {
        enabled: true,
        actions: vec![Scope::KeyboardBrightness],
        osd: true,
    };
    assert!(!settings.actions.contains(&scope));
    settings.actions = vec![scope];
    let mut facts = facts();
    assert!(authorization(&settings, &facts).unwrap().is_some());
    facts
        .capabilities
        .iter_mut()
        .find(|cap| cap.scope == scope)
        .unwrap()
        .identity = None;
    assert!(authorization(&settings, &facts).is_err());
    settings.actions = vec![Scope::KeyboardBrightness];
    assert!(authorization(&settings, &facts).unwrap().is_some());
}

#[test]
fn microphone_scope_does_not_inherit_keyboard_or_console_authorization() {
    let scope = Scope::for_action(ccc_core::oem_events::Action::ToggleMicrophones);
    assert_eq!(scope, Scope::MicrophoneMute);
    let mut settings = Settings {
        enabled: true,
        actions: vec![Scope::KeyboardBrightness, Scope::OpenConsole],
        osd: true,
    };
    assert!(!settings.actions.contains(&scope));
    settings.actions = vec![scope];
    let mut facts = facts();
    assert!(authorization(&settings, &facts).unwrap().is_some());
    facts
        .capabilities
        .iter_mut()
        .find(|cap| cap.scope == scope)
        .unwrap()
        .identity = None;
    assert!(authorization(&settings, &facts).is_err());
}

#[test]
fn touchpad_requires_its_own_authorized_device_scope() {
    let scope = Scope::for_action(ccc_core::oem_events::Action::ToggleTouchpad);
    assert_eq!(scope, Scope::Touchpad);
    let settings = Settings {
        enabled: true,
        actions: vec![Scope::MicrophoneMute, Scope::OpenConsole],
        osd: true,
    };
    assert!(!settings.actions.contains(&scope));
    let settings = Settings {
        actions: vec![scope],
        ..settings
    };
    let mut facts = facts();
    assert!(authorization(&settings, &facts).unwrap().is_some());
    facts
        .capabilities
        .iter_mut()
        .find(|c| c.scope == scope)
        .unwrap()
        .identity = None;
    assert!(authorization(&settings, &facts).is_err());
}

#[test]
fn keyboard_zone_scope_is_not_inherited_from_brightness_or_master_switch() {
    let scope = Scope::for_action(ccc_core::oem_events::Action::ToggleKeyboardZone(1));
    assert_eq!(scope, Scope::KeyboardZones);
    let settings = Settings {
        enabled: true,
        actions: vec![Scope::KeyboardBrightness, Scope::KeyboardSwitch],
        osd: true,
    };
    assert!(!settings.actions.contains(&scope));
    let settings = Settings {
        actions: vec![scope],
        ..settings
    };
    let mut facts = facts();
    assert!(authorization(&settings, &facts).unwrap().is_some());
    facts
        .capabilities
        .iter_mut()
        .find(|c| c.scope == scope)
        .unwrap()
        .identity = None;
    assert!(authorization(&settings, &facts).is_err());
}

#[path = "battery_cold/authorization_tests.rs"]
mod battery_cold_authorization_tests;
#[path = "brightness_event_tests.rs"]
mod brightness_event_tests;
#[path = "camera_report/authorization_tests.rs"]
mod camera_authorization_tests;
#[path = "mode_authorization_tests.rs"]
mod mode_authorization_tests;
#[path = "panel_hotkey/authorization_tests.rs"]
mod panel_hotkey_authorization_tests;
#[path = "pd_event_authorization_tests.rs"]
mod pd_event_authorization_tests;
#[path = "screen_brightness/authorization_tests.rs"]
mod screen_brightness_authorization_tests;
#[path = "system_action/authorization_tests.rs"]
mod system_action_authorization_tests;

#[test]
fn recovery_scope_is_independent_and_program_changes_revoke_its_grant() {
    let mut facts = facts();
    let mut settings = enabled();
    settings.actions = vec![Scope::SystemRecovery];
    let record = Record::authorize(settings, &facts).unwrap();
    record.verify_action(&facts, Scope::SystemRecovery).unwrap();
    assert!(record.verify_action(&facts, Scope::OpenConsole).is_err());
    assert_eq!(
        Scope::for_action(ccc_core::oem_events::Action::OpenRecovery),
        Scope::SystemRecovery
    );
    facts
        .capabilities
        .iter_mut()
        .find(|cap| cap.scope == Scope::SystemRecovery)
        .unwrap()
        .identity = Some("changed-control-program".into());
    assert!(record.verify_action(&facts, Scope::SystemRecovery).is_err());
}

#[path = "event_tests.rs"]
mod event_tests;
