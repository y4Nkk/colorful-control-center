use super::{
    test_fixture::{self, EnvironmentFixture},
    *,
};
use crate::gpuctl::GpuController;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

fn display() -> crate::display::Controller {
    crate::display::test_fixture::Fixture::new().controller
}
#[test]
fn unchanged_target_has_no_authorization() {
    let environment = EnvironmentFixture::new();
    let controller = Controller::with_environment(environment.clone());
    let hal = test_fixture::hal();
    let clocks = GpuController::new();
    controller
        .apply_runtime(&hal, &clocks, &display(), &environment.io.device(), true)
        .unwrap();
    assert_eq!(environment.authorizations.load(Ordering::SeqCst), 0);
    assert!(environment.io.0.lock().unwrap().events.is_empty());
}
#[test]
fn denied_authorization_has_no_device_output() {
    let environment = EnvironmentFixture::new();
    environment.io.0.lock().unwrap().deny = true;
    let controller = Controller::with_environment(environment.clone());
    let hal = test_fixture::hal();
    let clocks = GpuController::new();
    assert!(
        controller
            .apply_runtime(&hal, &clocks, &display(), &environment.io.device(), false)
            .is_err()
    );
    assert!(environment.io.0.lock().unwrap().events.is_empty());
    assert_eq!(environment.prepared.load(Ordering::SeqCst), 0);
}
#[test]
fn authorization_wait_releases_hal_and_changed_device_refuses_permission() {
    let environment = EnvironmentFixture::new();
    let controller = Controller::with_environment(environment.clone());
    let hal = Arc::new(test_fixture::hal());
    let unlocked = Arc::new(AtomicBool::new(false));
    let check = unlocked.clone();
    let shared = hal.clone();
    let io = environment.io.clone();
    environment.io.0.lock().unwrap().authorize_hook = Some(Box::new(move || {
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _gate = shared.begin_write().unwrap();
            let _ = sender.send(());
        });
        check.store(
            receiver
                .recv_timeout(std::time::Duration::from_secs(1))
                .is_ok(),
            Ordering::SeqCst,
        );
        io.0.lock().unwrap().device.drives_display = true;
    }));
    let clocks = GpuController::new();
    assert!(
        controller
            .apply_runtime(&hal, &clocks, &display(), &environment.io.device(), false)
            .is_err()
    );
    assert!(unlocked.load(Ordering::SeqCst));
    assert_eq!(environment.prepared.load(Ordering::SeqCst), 0);
    assert!(environment.io.0.lock().unwrap().events.is_empty());
}
#[test]
fn retained_scope_restores_later_failure_without_reauthorization() {
    let environment = EnvironmentFixture::new();
    let controller = Controller::with_environment(environment.clone());
    let before = environment.io.device();
    let hal = test_fixture::hal();
    let clocks = GpuController::new();
    let mut scope = controller.scope().unwrap();
    let request = {
        let _gate = hal.begin_write().unwrap();
        scope
            .preview(&hal, &before, Target::requested(false))
            .unwrap()
            .unwrap()
    };
    scope.prepare(std::slice::from_ref(&request)).unwrap();
    {
        let _gate = hal.begin_write().unwrap();
        scope.execute(&hal, &clocks, &request).unwrap();
        assert!(hal.gpu_suspended());
        scope.rollback(&hal, &clocks).unwrap();
        assert!(!hal.gpu_suspended());
    }
    scope.finish().unwrap();
    assert_eq!(environment.io.device(), before);
    assert_eq!(environment.authorizations.load(Ordering::SeqCst), 1);
}
#[test]
fn isolated_default_controller_refuses_native_environment() {
    let controller = Controller::default();
    assert!(
        controller
            .read_runtime(&test_fixture::hal(), &GpuController::new())
            .unwrap_err()
            .to_string()
            .contains("禁止访问真实设备")
    );
}

#[test]
fn replay_request_replays_firmware_when_readable_flags_already_match() {
    let environment = EnvironmentFixture::new();
    let controller = Controller::with_environment(environment.clone());
    let before = environment.io.device();
    environment.io.0.lock().unwrap().firmware = false;
    let hal = test_fixture::hal();
    let clocks = GpuController::new();
    let mut scope = controller.scope().unwrap();
    let request = request::Request {
        expected: before.clone(),
        target: before.target(),
        restore: true,
    };
    scope.prepare(std::slice::from_ref(&request)).unwrap();
    {
        let _gate = hal.begin_write().unwrap();
        scope.execute(&hal, &clocks, &request).unwrap();
    }
    scope.finish().unwrap();
    assert!(environment.io.0.lock().unwrap().firmware);
    assert_eq!(environment.authorizations.load(Ordering::SeqCst), 1);
}
#[test]
fn refresh_trial_started_during_authorization_blocks_all_device_preparation() {
    let environment = EnvironmentFixture::new();
    let controller = Controller::with_environment(environment.clone());
    let fixture = crate::display::test_fixture::Fixture::new();
    fixture.enable();
    let display = Arc::new(fixture.controller);
    let captured = display.clone();
    environment.io.0.lock().unwrap().authorize_hook = Some(Box::new(move || {
        let before = captured.read().unwrap().displays[0].current.clone();
        captured.apply(&before, 144, 0).unwrap();
    }));
    let result = controller.apply_runtime(
        &test_fixture::hal(),
        &GpuController::new(),
        &display,
        &environment.io.device(),
        false,
    );
    assert!(result.unwrap_err().to_string().contains("刷新率试用"));
    assert_eq!(environment.prepared.load(Ordering::SeqCst), 0);
    assert!(environment.io.0.lock().unwrap().events.is_empty());
}
