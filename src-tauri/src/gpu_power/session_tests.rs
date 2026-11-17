use super::sequence::Sequence;
use super::{
    execution::Execution,
    request::{self, Invocation},
    test_fixture::Memory,
    *,
};
use crate::hal::authorization::session::Task;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[test]
fn authorization_ready_is_device_free_and_rollback_uses_same_process() {
    let io = Memory::default();
    let before = io.device();
    let copy = io.clone();
    let prepared = Arc::new(AtomicUsize::new(0));
    let count = prepared.clone();
    let mut session = request::prepare_with(
        Sequence::new(&[io.request(false)]).unwrap(),
        move |arguments| {
            let invocation = Invocation::from_args(
                &arguments.split(' ').map(str::to_owned).collect::<Vec<_>>(),
            )?;
            request::serve(&invocation, move |request| {
                count.fetch_add(1, Ordering::SeqCst);
                Ok(Box::new(Execution::new(copy.clone(), request)))
            })?;
            Ok(0)
        },
    )
    .unwrap();
    assert_eq!(prepared.load(Ordering::SeqCst), 0);
    assert!(io.0.lock().unwrap().events.is_empty());
    session.execute().unwrap();
    assert!(!io.device().enabled);
    session.rollback().unwrap();
    assert_eq!(io.device(), before);
    assert_eq!(prepared.load(Ordering::SeqCst), 1);
    assert!(!session.dirty());
    assert!(session.execute().is_err());
    session.finish().unwrap();
}
#[test]
fn execution_compensation_preserves_mixed_policy_and_rejects_foreign_state() {
    let io = Memory::default();
    io.0.lock().unwrap().device.automatic_power_off = true;
    let before = io.device();
    let mut task = Execution::new(io.clone(), io.request(false));
    task.execute().unwrap();
    task.rollback().unwrap();
    assert_eq!(io.device(), before);
    let mut task = Execution::new(io.clone(), io.request(false));
    task.execute().unwrap();
    io.0.lock().unwrap().device.automatic_power_off = false;
    let writes = io.0.lock().unwrap().events.len();
    assert!(
        task.rollback()
            .unwrap_err()
            .to_string()
            .contains("外部修改")
    );
    assert!(task.dirty());
    assert_eq!(io.0.lock().unwrap().events.len(), writes);
}
#[test]
fn full_device_change_or_unknown_request_never_executes() {
    let io = Memory::default();
    let request = io.request(false);
    let mut task = Execution::new(io.clone(), request.clone());
    io.0.lock().unwrap().device.name = "replaced".into();
    assert!(task.execute().is_err());
    assert!(!task.dirty());
    assert!(io.0.lock().unwrap().events.is_empty());
    let mut extra = serde_json::to_value(request).unwrap();
    extra["old"] = true.into();
    assert!(serde_json::from_value::<request::Request>(extra).is_err());
    assert!(
        Invocation::from_args(&[
            "--oem-gpu-power".into(),
            "ACPI\\NVDA0820\\A".into(),
            "1".into(),
            "0".into(),
            "0".into(),
            "1".into()
        ])
        .is_err()
    );
}
#[test]
fn request_change_after_ready_is_rejected_before_device_preparation() {
    let io = Memory::default();
    let copy = io.clone();
    let slot = Arc::new(std::sync::Mutex::new(None));
    let writer = slot.clone();
    let mut session = request::prepare_with(
        Sequence::new(&[io.request(false)]).unwrap(),
        move |arguments| {
            let invocation = Invocation::from_args(
                &arguments.split(' ').map(str::to_owned).collect::<Vec<_>>(),
            )?;
            *writer.lock().unwrap() = Some(invocation.session_dir.clone());
            request::serve(&invocation, |_| bail!("请求变化后不能准备设备"))?;
            Ok(0)
        },
    )
    .unwrap();
    std::fs::write(
        slot.lock().unwrap().as_ref().unwrap().join("request.json"),
        b"{}",
    )
    .unwrap();
    assert!(
        session
            .execute()
            .unwrap_err()
            .to_string()
            .contains("请求复核失败")
    );
    assert!(!session.dirty());
    session.finish().unwrap();
    assert!(copy.0.lock().unwrap().events.is_empty());
}
#[test]
fn cancelling_applied_session_restores_without_a_second_authorization() {
    let io = Memory::default();
    let before = io.device();
    let copy = io.clone();
    let mut session = request::prepare_with(
        Sequence::new(&[io.request(false)]).unwrap(),
        move |arguments| {
            let invocation = Invocation::from_args(
                &arguments.split(' ').map(str::to_owned).collect::<Vec<_>>(),
            )?;
            request::serve(&invocation, move |request| {
                Ok(Box::new(Execution::new(copy.clone(), request)))
            })?;
            Ok(0)
        },
    )
    .unwrap();
    session.execute().unwrap();
    drop(session);
    assert_eq!(io.device(), before);
}

#[test]
fn each_sequence_step_rechecks_the_frozen_file_and_keeps_original_recovery_available() {
    let io = Memory::default();
    let original = io.device();
    let first = io.request(false);
    let mut after = original.clone();
    after.enabled = false;
    after.problem = 22;
    after.automatic_power_off = true;
    let second = request::Request {
        expected: after,
        target: original.target(),
        restore: false,
    };
    let slot = Arc::new(std::sync::Mutex::new(None));
    let writer = slot.clone();
    let copy = io.clone();
    let count = Arc::new(AtomicUsize::new(0));
    let prepared = count.clone();
    let mut session =
        request::prepare_with(Sequence::new(&[first, second]).unwrap(), move |arguments| {
            let invocation = Invocation::from_args(
                &arguments.split(' ').map(str::to_owned).collect::<Vec<_>>(),
            )?;
            *writer.lock().unwrap() = Some(invocation.session_dir.clone());
            request::serve(&invocation, move |request| {
                prepared.fetch_add(1, Ordering::SeqCst);
                Ok(Box::new(Execution::new(copy.clone(), request)))
            })?;
            Ok(0)
        })
        .unwrap();
    session.execute().unwrap();
    let outputs = io.0.lock().unwrap().events.len();
    std::fs::write(
        slot.lock().unwrap().as_ref().unwrap().join("request.json"),
        b"{}",
    )
    .unwrap();
    assert!(format!("{:#}", session.execute().unwrap_err()).contains("请求复核失败"));
    assert_eq!(io.0.lock().unwrap().events.len(), outputs);
    assert_eq!(count.load(Ordering::SeqCst), 1);
    session.rollback().unwrap();
    assert_eq!(io.device(), original);
    session.finish().unwrap();
}
#[test]
fn local_write_failure_is_reported_with_detail_and_compensated_once() {
    let io = Memory::default();
    let before = io.device();
    io.0.lock().unwrap().failures = vec![4];
    let mut task = Execution::new(io.clone(), io.request(false));
    let error = task.execute().unwrap_err().to_string();
    assert!(error.contains("隔离独显写入失败"));
    assert!(!task.dirty());
    let writes = io.0.lock().unwrap().events.len();
    task.rollback().unwrap();
    assert_eq!(writes, io.0.lock().unwrap().events.len());
    assert_eq!(io.device(), before);
}

#[test]
fn failed_firmware_recovery_is_replayed_even_when_node_and_policy_match_source() {
    let io = Memory::default();
    {
        let mut data = io.0.lock().unwrap();
        data.device.enabled = false;
        data.device.problem = 22;
        data.device.automatic_power_off = true;
        data.firmware = false;
        data.failures = vec![1, 5];
    }
    let before = io.device();
    let mut task = Execution::new(io.clone(), io.request(true));
    assert!(task.execute().is_err());
    assert!(task.dirty());
    assert_eq!(io.device(), before);
    let failed_writes = io.0.lock().unwrap().events.len();
    task.rollback().unwrap();
    assert!(io.0.lock().unwrap().events.len() > failed_writes);
    assert!(!io.0.lock().unwrap().firmware);
    assert!(!task.dirty());
}

#[test]
fn retained_recovery_never_disables_an_active_display_output() {
    let io = Memory::default();
    {
        let mut data = io.0.lock().unwrap();
        data.device.enabled = false;
        data.device.problem = 22;
        data.device.automatic_power_off = true;
        data.firmware = false;
    }
    let mut task = Execution::new(io.clone(), io.request(true));
    task.execute().unwrap();
    {
        let mut data = io.0.lock().unwrap();
        data.device.drives_display = true;
        data.device.can_disable = false;
    }
    let before = io.0.lock().unwrap().events.len();
    assert!(task.rollback().is_err());
    assert_eq!(io.0.lock().unwrap().events.len(), before);
    assert!(task.dirty());
}
