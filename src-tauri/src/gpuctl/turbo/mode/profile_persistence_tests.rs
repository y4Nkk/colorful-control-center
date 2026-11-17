use super::*;
use crate::profiles::{
    application::Permission,
    policy::{Controller, Record, Request},
};
use std::{fs, fs::OpenOptions, os::windows::fs::OpenOptionsExt, path::Path};
use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ;

fn configured(h: &Harness, preview: &Preview, dir: &Path) -> Controller {
    let path = dir.join("power-profile-policy.json");
    let controller = Controller::new(path);
    controller.test_ready().unwrap();
    fs::write(
        dir.join("完整模式档案.json"),
        serde_json::to_vec(&preview.file).unwrap(),
    )
    .unwrap();
    let session = context(&h.f, &h.memory, &h.gpu, &h.power)
        .session()
        .unwrap();
    let facts = {
        let _gate = h.f.hal.begin_write().unwrap();
        crate::profiles::preview_authorization(&session.context, &preview.file.settings)
            .unwrap()
            .unwrap()
    };
    let request: Request = serde_json::from_value(serde_json::json!({
        "expected":controller.read().unwrap().record.unwrap(), "enabled":true,
        "ac":{"file":preview.file,"mode":facts},"battery":null,"resume":null,
    }))
    .unwrap();
    controller.save(&session.context, dir, &request).unwrap();
    controller
}
fn apply(
    h: &Harness,
    preview: &Preview,
    permission: &Permission,
) -> Vec<crate::profiles::ApplyStep> {
    crate::profiles::apply_reviewed(
        &h.f.hal,
        &h.gpu,
        h.devices(),
        &preview.file,
        &preview.file,
        Some(&preview.mode),
        Some(&preview.power),
        Some(permission),
    )
    .unwrap()
}
fn driver(controller: &Controller) -> Option<String> {
    controller
        .read()
        .unwrap()
        .record
        .unwrap()
        .ac
        .unwrap()
        .mode
        .unwrap()
        .driver
}

fn startup_permission(h: &Harness, preview: &Preview, dir: &Path) -> Permission {
    let session = context(&h.f, &h.memory, &h.gpu, &h.power)
        .session()
        .unwrap();
    let facts = {
        let _gate = h.f.hal.begin_write().unwrap();
        crate::profiles::preview_authorization(&session.context, &preview.file.settings)
            .unwrap()
            .unwrap()
    };
    let binding = crate::profiles::authorization::Binding::reviewed(
        &preview.file,
        Some(facts.grant().unwrap()),
    )
    .unwrap();
    fs::write(
        dir.join("完整模式档案.json"),
        serde_json::to_vec(&preview.file).unwrap(),
    )
    .unwrap();
    crate::startup::test_permission(dir, binding).unwrap()
}

#[test]
fn startup_driver_is_persisted_before_output_and_the_same_permission_remains_valid() {
    let h = Harness::new(true);
    let preview = h.preview();
    let dir = tempfile::tempdir().unwrap();
    let permission = startup_permission(&h, &preview, dir.path());
    let path = dir.path().join("startup-profile.json");
    let observed = path.clone();
    h.memory.0.lock().unwrap().write_observer = Some(Arc::new(move || {
        let record: serde_json::Value =
            serde_json::from_slice(&fs::read(&observed).unwrap()).unwrap();
        assert_eq!(record["version"], 4);
        assert!(record["profile"]["mode"]["driver"].is_string());
    }));
    success(&apply(&h, &preview, &permission));
    permission.check().unwrap();
    let value: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(value["profile"]["mode"].as_object().unwrap().len(), 4);
    assert!(value["profile"].get("settings").is_none());
}

#[test]
fn startup_driver_save_failure_has_no_gpu_output_preserves_record_and_retries() {
    let h = Harness::new(true);
    let preview = h.preview();
    let dir = tempfile::tempdir().unwrap();
    let permission = startup_permission(&h, &preview, dir.path());
    let path = dir.path().join("startup-profile.json");
    let before = fs::read(&path).unwrap();
    let locked = OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .open(&path)
        .unwrap();
    let steps = apply(&h, &preview, &permission);
    assert!(
        steps
            .iter()
            .any(|step| !step.ok && step.message.contains("记录启动档案GPU驱动失败"))
    );
    assert_eq!(fs::read(&path).unwrap(), before);
    assert!(h.memory.0.lock().unwrap().writes.is_empty());
    assert!(h.f.device.0.lock().unwrap().events.is_empty());
    assert_eq!(h.environment.io.device(), preview.power.before);
    drop(locked);
    success(&apply(&h, &h.preview(), &permission));
    permission.check().unwrap();
}

#[test]
fn startup_selection_revoked_during_system_authorization_has_no_hardware_output() {
    let h = Harness::new(true);
    let preview = h.preview();
    let dir = tempfile::tempdir().unwrap();
    let permission = startup_permission(&h, &preview, dir.path());
    let path = dir.path().join("startup-profile.json");
    h.environment.io.0.lock().unwrap().authorize_hook = Some(Box::new(move || {
        fs::write(&path, br#"{"version":4,"profile":null}"#).unwrap();
    }));
    assert!(
        crate::profiles::apply_reviewed(
            &h.f.hal,
            &h.gpu,
            h.devices(),
            &preview.file,
            &preview.file,
            Some(&preview.mode),
            Some(&preview.power),
            Some(&permission)
        )
        .is_err()
    );
    assert!(h.environment.io.0.lock().unwrap().events.is_empty());
    assert!(h.memory.0.lock().unwrap().writes.is_empty());
    assert!(h.f.device.0.lock().unwrap().events.is_empty());
}

#[test]
fn first_deferred_driver_is_on_disk_before_gpu_output_and_does_not_revoke_its_token() {
    let h = Harness::new(true);
    let preview = h.preview();
    let dir = tempfile::tempdir().unwrap();
    let controller = configured(&h, &preview, dir.path());
    assert!(driver(&controller).is_none());
    let path = dir.path().join("power-profile-policy.json");
    h.memory.0.lock().unwrap().write_observer = Some(Arc::new(move || {
        let record: Record = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert!(record.ac.unwrap().mode.unwrap().driver.is_some());
    }));
    let permission = controller.test_ac_permission(0).unwrap();
    success(&apply(&h, &preview, &permission));
    assert!(driver(&controller).is_some());
    permission.check().unwrap();
    assert_eq!(h.environment.authorizations.load(Ordering::SeqCst), 1);
}

#[test]
fn failed_driver_persistence_restores_preparation_and_the_same_token_can_retry() {
    let h = Harness::new(true);
    let preview = h.preview();
    let dir = tempfile::tempdir().unwrap();
    let controller = configured(&h, &preview, dir.path());
    let path = dir.path().join("power-profile-policy.json");
    let before = fs::read(&path).unwrap();
    let permission = controller.test_ac_permission(0).unwrap();
    let locked = OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .open(&path)
        .unwrap();
    let steps = apply(&h, &preview, &permission);
    assert!(
        steps
            .iter()
            .any(|step| !step.ok && step.message.contains("记录自动档案GPU驱动失败"))
    );
    assert_eq!(fs::read(&path).unwrap(), before);
    assert_eq!(h.environment.io.device(), preview.power.before);
    assert!(h.memory.0.lock().unwrap().writes.is_empty());
    assert!(h.f.device.0.lock().unwrap().events.is_empty());
    permission.check().unwrap();
    drop(locked);
    success(&apply(&h, &h.preview(), &permission));
    assert!(driver(&controller).is_some());
    permission.check().unwrap();
}

#[test]
fn a_later_known_driver_change_has_no_output_and_never_replaces_the_first_identity() {
    let h = Harness::new(true);
    let preview = h.preview();
    let dir = tempfile::tempdir().unwrap();
    let controller = configured(&h, &preview, dir.path());
    let first = controller.test_ac_permission(0).unwrap();
    success(&apply(&h, &preview, &first));
    let original = driver(&controller).unwrap();
    h.memory.0.lock().unwrap().offsets[0].core.max_khz -= 1000;
    let writes = h.memory.0.lock().unwrap().writes.len();
    let events = h.f.device.0.lock().unwrap().events.len();
    let next = controller.test_ac_permission(10_000).unwrap();
    assert!(first.check().is_err());
    let steps = apply(&h, &h.preview(), &next);
    assert!(
        steps
            .iter()
            .any(|step| !step.ok && step.message.contains("驱动"))
    );
    assert_eq!(driver(&controller).as_deref(), Some(original.as_str()));
    assert_eq!(h.memory.0.lock().unwrap().writes.len(), writes);
    assert_eq!(h.f.device.0.lock().unwrap().events.len(), events);
}
