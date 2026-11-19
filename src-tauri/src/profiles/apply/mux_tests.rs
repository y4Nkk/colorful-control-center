use super::*;
use crate::graphics_mode;
use std::sync::Arc;

fn setup() -> Fixture {
    let f = Fixture::new();
    f.graphics_mode.io.0.lock().unwrap().supported = true;
    f
}
fn target(f: &Fixture, mode: u8) -> ProfileSettings {
    let scope = f.graphics_mode.controller.scope().unwrap();
    let _gate = f.hal.begin_write().unwrap();
    let mut target = scope.capture(&f.hal).unwrap().unwrap();
    target.mode = mode;
    ProfileSettings {
        graphics_mode: Some(target),
        pl1_w: Some(77.0),
        ..Default::default()
    }
}
fn review(f: &Fixture, settings: &ProfileSettings) -> graphics_mode::profile::Review {
    let scope = f.graphics_mode.controller.scope().unwrap();
    let _gate = f.hal.begin_write().unwrap();
    scope
        .preview_profile(&f.hal, settings.graphics_mode.as_ref().unwrap())
        .unwrap()
}
fn selection(f: &Fixture) -> graphics_mode::Profile {
    f.graphics_mode.controller.read(&f.hal).unwrap()
}
fn request(f: &Fixture, mode: u8) {
    f.graphics_mode
        .controller
        .apply(
            &f.hal,
            &f.display.controller,
            &graphics_mode::Request {
                expected_revision: selection(f).revision,
                target: mode,
            },
        )
        .unwrap();
}

#[test]
fn capture_uses_known_selection_without_creating_a_record() {
    let f = setup();
    let first = crate::profiles::capture_current(&f.hal, &f.gpu, f.devices(), &crate::profiles::ProfileGroup::ALL).unwrap();
    assert_eq!(first.graphics_mode.unwrap().mode, 3);
    assert!(!f.graphics_mode.dir.path().join("mode.json").exists());
    request(&f, 2);
    let captured = crate::profiles::capture_current(&f.hal, &f.gpu, f.devices(), &crate::profiles::ProfileGroup::ALL).unwrap();
    assert_eq!(captured.graphics_mode.as_ref().unwrap().mode, 2);
    assert_eq!(selection(&f).current.unwrap().mode, 3);
    let json = serde_json::to_string(&captured).unwrap();
    assert!(serde_json::from_str::<ProfileSettings>(&json).unwrap() == captured);
}

#[test]
fn profile_selection_is_pending_reboot_and_shares_initial_restore() {
    let f = setup();
    request(&f, 2);
    let steps = apply_settings(&f.hal, &f.gpu, f.devices(), &target(&f, 4)).unwrap();
    assert!(steps.iter().all(|step| step.ok));
    let state = selection(&f);
    let previous = state.last_request.unwrap();
    assert_eq!((previous.initial_mode, previous.target), (3, 4));
    assert_eq!(
        previous.verification,
        graphics_mode::Verification::PendingReboot
    );
    f.graphics_mode
        .controller
        .restore(&f.hal, &f.display.controller, &state.revision)
        .unwrap();
    let data = f.graphics_mode.io.0.lock().unwrap();
    assert_eq!(data.sent, [2, 4, 3]);
    assert_eq!(data.reboots, 0);
}

#[test]
fn owner_candidate_and_unsupported_changes_prevent_all_writes() {
    for change in 0..3 {
        let f = setup();
        let settings = target(&f, 2);
        {
            let mut data = f.graphics_mode.io.0.lock().unwrap();
            match change {
                0 => data.identity.owner = "b".repeat(64),
                1 => data.observed.available_mask = 7,
                _ => data.supported = false,
            }
        }
        assert!(apply_settings(&f.hal, &f.gpu, f.devices(), &settings).is_err());
        assert!(f.device.0.lock().unwrap().events.is_empty());
        assert!(f.graphics_mode.io.0.lock().unwrap().sent.is_empty());
    }
}

#[test]
fn partial_mux_failure_restores_prior_choice_and_saved_cpu() {
    let f = setup();
    request(&f, 2);
    f.graphics_mode.io.0.lock().unwrap().fail_send = vec![2];
    let before = f.device.0.lock().unwrap().pages[6];
    let steps = apply_settings(&f.hal, &f.gpu, f.devices(), &target(&f, 4)).unwrap();
    assert!(
        steps
            .iter()
            .any(|step| !step.ok && step.step.contains("显卡模式"))
    );
    assert!(
        steps
            .iter()
            .any(|step| step.ok && step.step.contains("恢复本次显卡选择"))
    );
    assert_eq!(f.device.0.lock().unwrap().pages[6], before);
    let state = selection(&f).last_request.unwrap();
    assert_eq!((state.initial_mode, state.target), (3, 2));
    assert_eq!(f.graphics_mode.io.0.lock().unwrap().sent, [2, 4, 2]);
}

#[test]
fn compensation_failure_is_reported_and_standalone_restore_can_retry() {
    let f = setup();
    f.graphics_mode.io.0.lock().unwrap().fail_send = vec![1, 2];
    let steps = apply_settings(&f.hal, &f.gpu, f.devices(), &target(&f, 2)).unwrap();
    assert!(
        steps
            .iter()
            .any(|step| !step.ok && step.step.contains("恢复本次显卡选择"))
    );
    let state = selection(&f);
    assert!(state.can_restore);
    f.graphics_mode
        .controller
        .restore(&f.hal, &f.display.controller, &state.revision)
        .unwrap();
    assert_eq!(selection(&f).last_request.unwrap().target, 3);
}

#[test]
fn final_display_fence_failure_restores_mux_and_cpu() {
    let f = setup();
    f.display.enable();
    let mut settings = target(&f, 2);
    settings.display = f.display.controller.capture().unwrap();
    let display = f.display.io.clone();
    f.graphics_mode.io.0.lock().unwrap().hook = Some(Arc::new(move |mode| {
        if mode == 2 {
            display.0.lock().unwrap().displays[0].current.frequency = 144;
        }
    }));
    let before = f.device.0.lock().unwrap().pages[6];
    let steps = apply_settings(&f.hal, &f.gpu, f.devices(), &settings).unwrap();
    assert!(
        steps
            .iter()
            .any(|step| !step.ok && step.step == "显示刷新率复核")
    );
    assert_eq!(f.graphics_mode.io.0.lock().unwrap().sent, [2, 3]);
    assert_eq!(f.device.0.lock().unwrap().pages[6], before);
    assert_eq!(
        f.display.io.0.lock().unwrap().displays[0].current.frequency,
        144
    );
}

#[test]
fn repeated_pending_target_is_a_noop() {
    let f = setup();
    request(&f, 2);
    let before = selection(&f).revision;
    let steps = apply_settings(&f.hal, &f.gpu, f.devices(), &target(&f, 2)).unwrap();
    assert!(steps.iter().all(|step| step.ok));
    assert_eq!(selection(&f).revision, before);
    assert_eq!(f.graphics_mode.io.0.lock().unwrap().sent, [2]);
}

#[test]
fn missing_injected_io_refuses_before_native_access() {
    let f = Fixture::new();
    let root = tempfile::tempdir().unwrap();
    let controller = graphics_mode::Controller::new(root.path().join("missing.json"));
    assert!(
        controller
            .read(&f.hal)
            .unwrap_err()
            .to_string()
            .contains("显式IO")
    );
    assert!(!root.path().join("missing.json").exists());
}

#[path = "mux_application_tests.rs"]
mod application;
#[path = "mux_process_tests.rs"]
mod process;
