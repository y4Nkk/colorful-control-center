use super::*;
use ccc_core::oem_events::{Action, CameraReport, Packet};

#[test]
fn camera_reports_never_inherit_notification_or_microphone_permissions() {
    let facts = facts();
    let action = Packet { code: 249, data: 0 }.action().unwrap();
    assert_eq!(
        action,
        Action::CameraReport(CameraReport::CameraMicrophone(true))
    );
    assert_eq!(Scope::for_action(action), Scope::CameraReports);
    for old_scope in [Scope::StateNotifications, Scope::MicrophoneMute] {
        let old = Record::authorize(
            Settings {
                enabled: true,
                actions: vec![old_scope],
                osd: true,
            },
            &facts,
        )
        .unwrap();
        old.verify(&facts).unwrap();
        assert!(!old.settings.actions.contains(&Scope::CameraReports));
    }
    let record = Record::authorize(
        Settings {
            enabled: true,
            actions: vec![Scope::CameraReports],
            osd: false,
        },
        &facts,
    )
    .unwrap();
    record.verify(&facts).unwrap();
    let mut changed = facts;
    changed
        .capabilities
        .iter_mut()
        .find(|c| c.scope == Scope::CameraReports)
        .unwrap()
        .identity = Some("other-camera-rules".into());
    assert!(record.verify(&changed).is_err());
}

#[test]
fn camera_policy_apply_failure_and_initial_restore_use_the_existing_software_transaction() {
    let io = Memory::new(false);
    let mut initial = None;
    let target = Settings {
        enabled: true,
        actions: vec![Scope::CameraReports],
        osd: false,
    };
    io.prepare_fail.set(true);
    assert!(
        transaction::change(
            &io,
            &io.revision(),
            |_, facts| Record::authorize(target.clone(), facts),
            &mut initial
        )
        .is_err()
    );
    assert_eq!(io.loaded.borrow().record.settings, Settings::default());
    assert!(!io.running.get());
    transaction::change(
        &io,
        &io.revision(),
        |_, facts| Record::authorize(target.clone(), facts),
        &mut initial,
    )
    .unwrap();
    assert_eq!(io.loaded.borrow().record.settings, target);
    assert!(io.running.get());
    let revision = io.restore_revision(initial.as_ref().unwrap());
    transaction::restore(&io, &revision, &mut initial).unwrap();
    assert_eq!(io.loaded.borrow().record.settings, Settings::default());
    assert!(!io.running.get());
}

#[test]
fn restoring_an_initially_enabled_camera_policy_resumes_that_policy_without_assuming_disabled() {
    let io = Memory::new(true);
    let original = Record::authorize(
        Settings {
            enabled: true,
            actions: vec![Scope::CameraReports],
            osd: false,
        },
        &facts(),
    )
    .unwrap();
    io.loaded.borrow_mut().record = original.clone();
    let mut initial = None;
    transaction::change(
        &io,
        &io.revision(),
        |_, _| Ok(Record::default()),
        &mut initial,
    )
    .unwrap();
    assert!(!io.running.get());
    let revision = io.restore_revision(initial.as_ref().unwrap());
    transaction::restore(&io, &revision, &mut initial).unwrap();
    assert!(io.running.get());
    assert_eq!(io.loaded.borrow().record, original);
}
