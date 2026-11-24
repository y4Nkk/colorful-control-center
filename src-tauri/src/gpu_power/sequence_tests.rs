use super::{
    request::Request,
    test_fixture::{self, EnvironmentFixture},
    *,
};
use crate::gpuctl::GpuController;
use std::sync::atomic::Ordering;

fn sequence(before: &Device, values: &[(bool, bool)]) -> Vec<Request> {
    let mut current = before.clone();
    values
        .iter()
        .map(|&(enabled, automatic)| {
            let target = Target {
                enabled,
                automatic_power_off: automatic,
            };
            let request = Request {
                expected: current.clone(),
                target,
                restore: false,
            };
            current.enabled = enabled;
            current.automatic_power_off = automatic;
            current.problem = if enabled { 0 } else { 22 };
            request
        })
        .collect()
}

#[test]
fn a_three_stage_sequence_is_authorized_before_output_and_each_repeated_request_executes_once() {
    let environment = EnvironmentFixture::new();
    {
        let mut data = environment.io.0.lock().unwrap();
        data.device.enabled = false;
        data.device.problem = 22;
        data.device.automatic_power_off = true;
        data.firmware = false;
    }
    let controller = Controller::with_environment(environment.clone());
    let hal = test_fixture::hal();
    let clocks = GpuController::new();
    let before = environment.io.device();
    let requests = sequence(&before, &[(true, true), (false, true), (true, true)]);
    assert_eq!(requests[0], requests[2]);
    let mut scope = controller.scope().unwrap();
    scope.prepare(&requests).unwrap();
    assert_eq!(environment.authorizations.load(Ordering::SeqCst), 1);
    assert_eq!(environment.prepared.load(Ordering::SeqCst), 0);
    assert!(environment.io.0.lock().unwrap().events.is_empty());
    let _gate = hal.begin_write().unwrap();
    for (index, request) in requests.iter().enumerate() {
        scope.execute(&hal, &clocks, request).unwrap();
        assert_eq!(environment.io.device().target(), request.target);
        assert_eq!(environment.prepared.load(Ordering::SeqCst), index + 1);
    }
    scope.rollback(&hal, &clocks).unwrap();
    assert_eq!(environment.io.device(), before);
    assert!(!environment.io.0.lock().unwrap().firmware);
    let outputs = environment.io.0.lock().unwrap().events.len();
    assert!(scope.execute(&hal, &clocks, &requests[0]).is_err());
    assert_eq!(environment.io.0.lock().unwrap().events.len(), outputs);
    scope.finish().unwrap();
}

#[test]
fn an_invalid_or_discontinuous_sequence_is_rejected_before_any_authorization() {
    let environment = EnvironmentFixture::new();
    let controller = Controller::with_environment(environment.clone());
    let before = environment.io.device();
    let valid = sequence(&before, &[(false, true), (true, false)]);
    let mut changed = valid.clone();
    changed[1].expected.name.push('x');
    let mut stale = valid.clone();
    stale[1].expected = before.clone();
    let mut altered = valid.clone();
    altered[1].expected.can_disable = false;
    for requests in [
        vec![],
        sequence(
            &before,
            &[(false, true), (true, false), (false, true), (true, true)],
        ),
        changed,
        stale,
        altered,
    ] {
        assert!(controller.scope().unwrap().prepare(&requests).is_err());
    }
    assert_eq!(environment.authorizations.load(Ordering::SeqCst), 0);
    assert!(environment.io.0.lock().unwrap().events.is_empty());
}

#[test]
fn cancelling_the_complete_sequence_has_no_device_output() {
    let environment = EnvironmentFixture::new();
    environment.io.0.lock().unwrap().deny = true;
    let controller = Controller::with_environment(environment.clone());
    let requests = sequence(&environment.io.device(), &[(false, true), (true, false)]);
    let mut scope = controller.scope().unwrap();
    assert!(scope.prepare(&requests).is_err());
    assert_eq!(environment.authorizations.load(Ordering::SeqCst), 1);
    assert_eq!(environment.prepared.load(Ordering::SeqCst), 0);
    assert!(environment.io.0.lock().unwrap().events.is_empty());
    scope.finish().unwrap();
}

#[test]
fn a_later_write_failure_restores_earlier_stages_and_blocks_all_further_execution() {
    let environment = EnvironmentFixture::new();
    let controller = Controller::with_environment(environment.clone());
    let hal = test_fixture::hal();
    let clocks = GpuController::new();
    let before = environment.io.device();
    let requests = sequence(&before, &[(false, true), (true, false), (false, false)]);
    let mut scope = controller.scope().unwrap();
    scope.prepare(&requests).unwrap();
    let _gate = hal.begin_write().unwrap();
    scope.execute(&hal, &clocks, &requests[0]).unwrap();
    {
        let mut data = environment.io.0.lock().unwrap();
        data.failures = vec![data.events.len() + 1];
    }
    assert!(scope.execute(&hal, &clocks, &requests[1]).is_err());
    assert_eq!(environment.io.device(), before);
    let outputs = environment.io.0.lock().unwrap().events.len();
    assert!(scope.execute(&hal, &clocks, &requests[2]).is_err());
    assert_eq!(environment.io.0.lock().unwrap().events.len(), outputs);
    assert_eq!(environment.prepared.load(Ordering::SeqCst), 2);
    scope.finish().unwrap();
}

#[test]
fn out_of_order_execution_cannot_skip_a_confirmed_stage() {
    let environment = EnvironmentFixture::new();
    let controller = Controller::with_environment(environment.clone());
    let hal = test_fixture::hal();
    let clocks = GpuController::new();
    let requests = sequence(&environment.io.device(), &[(false, true), (true, false)]);
    let mut scope = controller.scope().unwrap();
    scope.prepare(&requests).unwrap();
    let _gate = hal.begin_write().unwrap();
    assert!(scope.execute(&hal, &clocks, &requests[1]).is_err());
    assert_eq!(environment.prepared.load(Ordering::SeqCst), 0);
    assert!(environment.io.0.lock().unwrap().events.is_empty());
    assert!(scope.execute(&hal, &clocks, &requests[0]).is_err());
    scope.finish().unwrap();
}

#[test]
fn changed_device_after_an_earlier_step_is_not_overwritten_during_compensation() {
    let environment = EnvironmentFixture::new();
    let controller = Controller::with_environment(environment.clone());
    let hal = test_fixture::hal();
    let clocks = GpuController::new();
    let requests = sequence(&environment.io.device(), &[(false, true), (true, false)]);
    let mut scope = controller.scope().unwrap();
    scope.prepare(&requests).unwrap();
    let _gate = hal.begin_write().unwrap();
    scope.execute(&hal, &clocks, &requests[0]).unwrap();
    environment.io.0.lock().unwrap().device.automatic_power_off = false;
    let outputs = environment.io.0.lock().unwrap().events.len();
    let error = scope.execute(&hal, &clocks, &requests[1]).unwrap_err();
    assert!(format!("{error:#}").contains("外部修改"));
    assert_eq!(environment.io.0.lock().unwrap().events.len(), outputs);
    environment.io.0.lock().unwrap().device.automatic_power_off = true;
    scope.rollback(&hal, &clocks).unwrap();
    assert!(environment.io.device().enabled);
    scope.finish().unwrap();
}

#[test]
fn the_same_complete_sequence_tracks_attempts_and_reports_failed_finish() {
    let environment = EnvironmentFixture::new();
    let controller = Controller::with_environment(environment.clone());
    let hal = test_fixture::hal();
    let clocks = GpuController::new();
    let requests = sequence(&environment.io.device(), &[(false, true), (true, false)]);
    let mut scope = controller.scope().unwrap();
    scope.prepare(&requests).unwrap();
    assert_eq!(environment.authorizations.load(Ordering::SeqCst), 1);
    let _gate = hal.begin_write().unwrap();
    assert!(!scope.attempted());
    scope.execute(&hal, &clocks, &requests[0]).unwrap();
    assert!(scope.attempted());
    scope.execute(&hal, &clocks, &requests[1]).unwrap();
    assert!(scope.attempted());
    environment.io.0.lock().unwrap().fail_finish = true;
    let error = scope.finish().unwrap_err();
    let error = format!("{error:#}");
    assert!(error.contains("隔离独显授权收尾未确认"));
}

#[test]
fn all_failed_recovery_stages_are_reported_and_the_same_sessions_can_retry() {
    let environment = EnvironmentFixture::new();
    {
        let mut data = environment.io.0.lock().unwrap();
        data.device.enabled = false;
        data.device.problem = 22;
        data.device.automatic_power_off = true;
        data.firmware = false;
    }
    let controller = Controller::with_environment(environment.clone());
    let hal = test_fixture::hal();
    let clocks = GpuController::new();
    let before = environment.io.device();
    let requests = sequence(&before, &[(true, true), (false, true), (true, false)]);
    let mut scope = controller.scope().unwrap();
    scope.prepare(&requests).unwrap();
    let _gate = hal.begin_write().unwrap();
    for request in &requests {
        scope.execute(&hal, &clocks, request).unwrap();
    }
    {
        let mut data = environment.io.0.lock().unwrap();
        data.device.drives_display = true;
        data.device.can_disable = false;
    }
    let outputs = environment.io.0.lock().unwrap().events.len();
    let error = format!("{:#}", scope.rollback(&hal, &clocks).unwrap_err());
    for stage in [1, 2, 3] {
        assert!(error.contains(&format!("独显阶段{stage}")));
    }
    assert_eq!(environment.io.0.lock().unwrap().events.len(), outputs);
    {
        let mut data = environment.io.0.lock().unwrap();
        data.device.drives_display = false;
        data.device.can_disable = true;
    }
    scope.rollback(&hal, &clocks).unwrap();
    assert_eq!(environment.io.device(), before);
    assert_eq!(environment.authorizations.load(Ordering::SeqCst), 1);
    scope.finish().unwrap();
}

#[test]
fn dropping_the_single_authorized_scope_recovers_all_stages_before_releasing_ownership() {
    let environment = EnvironmentFixture::new();
    let controller = Controller::with_environment(environment.clone());
    let hal = test_fixture::hal();
    let clocks = GpuController::new();
    let before = environment.io.device();
    let requests = sequence(&before, &[(false, true), (true, true), (false, false)]);
    let mut scope = controller.scope().unwrap();
    scope.prepare(&requests).unwrap();
    let _gate = hal.begin_write().unwrap();
    for request in &requests {
        scope.execute(&hal, &clocks, request).unwrap();
    }
    drop(scope);
    assert_eq!(environment.io.device(), before);
    assert_eq!(environment.authorizations.load(Ordering::SeqCst), 1);
    controller.scope().unwrap();
}

#[test]
fn sequence_files_reject_the_old_single_request_and_unknown_fields() {
    let environment = EnvironmentFixture::new();
    let requests = sequence(&environment.io.device(), &[(false, true), (true, false)]);
    let old = serde_json::to_value(&requests[0]).unwrap();
    assert!(serde_json::from_value::<sequence::Sequence>(old).is_err());
    let mut current = serde_json::to_value(sequence::Sequence::new(&requests).unwrap()).unwrap();
    current["legacy"] = true.into();
    assert!(serde_json::from_value::<sequence::Sequence>(current).is_err());
    assert!(
        sequence::Sequence::new(&[Request {
            expected: environment.io.device(),
            target: environment.io.device().target(),
            restore: false
        }])
        .is_err()
    );
}

#[test]
fn partial_sequence_finish_cannot_commit_an_intermediate_target() {
    let environment = EnvironmentFixture::new();
    let controller = Controller::with_environment(environment.clone());
    let hal = test_fixture::hal();
    let clocks = GpuController::new();
    let before = environment.io.device();
    let requests = sequence(&before, &[(false, true), (true, true), (false, false)]);
    let mut scope = controller.scope().unwrap();
    scope.prepare(&requests).unwrap();
    let _gate = hal.begin_write().unwrap();
    scope.execute(&hal, &clocks, &requests[0]).unwrap();
    assert!(format!("{:#}", scope.finish().unwrap_err()).contains("不能提交部分目标"));
    assert_eq!(environment.io.device(), before);
}
