use super::super::{model::Request, worker::Intent};
use super::*;
use crate::hal::authorization::session::{
    events::Events,
    protocol::{self, Phase, Reply},
};
use anyhow::bail;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

struct Memory {
    value: Arc<AtomicBool>,
    dirty: bool,
}
impl Task for Memory {
    fn execute(&mut self) -> Result<()> {
        self.value.store(true, Ordering::SeqCst);
        self.dirty = true;
        Ok(())
    }
    fn rollback(&mut self) -> Result<()> {
        self.value.store(false, Ordering::SeqCst);
        self.dirty = false;
        Ok(())
    }
    fn dirty(&self) -> bool {
        self.dirty
    }
}
fn operation(path: std::path::PathBuf) -> Operation {
    Operation {
        intent: Intent::Apply {
            request: Request {
                expected_revision: "isolated".into(),
                device_id: "isolated-device".into(),
                power_saving: Some(true),
                apply_wake_patch: false,
            },
        },
        recovery_file: path,
    }
}
fn prepare(value: Arc<AtomicBool>) -> (Box<dyn Authorized>, tempfile::TempDir) {
    let root = tempfile::tempdir().unwrap();
    let session = prepare_with(
        operation(root.path().join("lan-recovery.json")),
        move |arguments| {
            let invocation = Invocation::from_args(
                &arguments.split(' ').map(str::to_owned).collect::<Vec<_>>(),
            )?;
            serve(&invocation, || {
                Ok(Box::new(Memory {
                    value,
                    dirty: false,
                }))
            })?;
            Ok(0)
        },
    )
    .unwrap();
    (session, root)
}
#[test]
fn worker_rejection_before_ready_reports_this_sessions_actual_reason() {
    let root = tempfile::tempdir().unwrap();
    let error = prepare_with(
        operation(root.path().join("lan-recovery.json")),
        |arguments| {
            let invocation = Invocation::from_args(
                &arguments.split(' ').map(str::to_owned).collect::<Vec<_>>(),
            )?;
            invocation.operation()?;
            report_failure(&invocation.session_dir, "隔离已有LAN授权工作进程")?;
            Ok(7)
        },
    )
    .err()
    .unwrap();
    assert!(error.to_string().contains("已有LAN授权工作进程"));
}
#[test]
fn failed_process_wait_preserves_session_diagnostics_after_finalized_targets() {
    let root = tempfile::tempdir().unwrap();
    let value = Arc::new(AtomicBool::new(false));
    let copied = value.clone();
    let mut session = prepare_with(
        operation(root.path().join("lan-recovery.json")),
        move |arguments| {
            let invocation = Invocation::from_args(
                &arguments.split(' ').map(str::to_owned).collect::<Vec<_>>(),
            )?;
            serve(&invocation, || {
                Ok(Box::new(Memory {
                    value: copied,
                    dirty: false,
                }))
            })?;
            bail!("隔离进程等待失败")
        },
    )
    .unwrap();
    session.execute().unwrap();
    assert!(session.finish().is_err());
    assert!(value.load(Ordering::SeqCst));
    assert!(
        std::fs::read_dir(root.path()).unwrap().any(|entry| entry
            .unwrap()
            .path()
            .join("request.json")
            .exists())
    );
}
#[test]
fn ready_does_not_authorize_writes_but_explicit_execution_and_finish_do() {
    let value = Arc::new(AtomicBool::new(false));
    let (mut session, _root) = prepare(value.clone());
    assert!(!value.load(Ordering::SeqCst));
    session.execute().unwrap();
    assert!(value.load(Ordering::SeqCst));
    assert!(session.dirty());
    assert!(session.execute().is_err());
    session.finish().unwrap();
    assert!(value.load(Ordering::SeqCst));
}
#[test]
fn same_authorization_can_restore_then_finish_without_relaunch() {
    let value = Arc::new(AtomicBool::new(false));
    let (mut session, _root) = prepare(value.clone());
    session.execute().unwrap();
    session.rollback().unwrap();
    assert!(!session.dirty());
    assert!(!value.load(Ordering::SeqCst));
    session.rollback().unwrap();
    session.finish().unwrap();
}
#[test]
fn dropping_ready_or_applied_sessions_cancels_and_reaps_without_committing_targets() {
    for applied in [false, true] {
        let value = Arc::new(AtomicBool::new(false));
        let (mut session, _root) = prepare(value.clone());
        if applied {
            session.execute().unwrap();
        }
        drop(session);
        assert!(!value.load(Ordering::SeqCst));
    }
}
#[test]
fn launch_denial_and_exit_before_ready_are_not_execution() {
    let root = tempfile::tempdir().unwrap();
    assert!(
        prepare_with(operation(root.path().join("lan-recovery.json")), |_| bail!(
            "隔离授权取消"
        ))
        .is_err()
    );
    assert!(prepare_with(operation(root.path().join("lan-recovery.json")), |_| Ok(0)).is_err());
}
#[test]
fn cancellation_has_priority_over_an_old_execution_event() {
    let root = tempfile::tempdir().unwrap();
    let binding = Binding::new().unwrap();
    let events = Events::create(&binding).unwrap();
    let directory = tempfile::Builder::new()
        .prefix(&format!(".lan-session-{}-", binding.token))
        .tempdir_in(root.path())
        .unwrap();
    let bytes = operation(root.path().join("lan-recovery.json"))
        .encoded()
        .unwrap();
    std::fs::write(directory.path().join("request.json"), &bytes).unwrap();
    let invocation = Invocation {
        session_dir: directory.path().into(),
        request_hash: hash_bytes(&bytes),
        authorization: binding.clone(),
    };
    events.cancel.signal().unwrap();
    events.execute.signal().unwrap();
    serve(&invocation, || bail!("取消后不能创建任务")).unwrap();
    assert!(!directory.path().join("response.json").exists());
    assert!(Events::create(&binding).is_err());
}
#[test]
fn binding_legacy_arguments_and_reply_phase_sequence_are_strict() {
    for token in ["", "xyz", "../../outside"] {
        assert!(
            Binding {
                parent_id: 1,
                token: token.into()
            }
            .validate()
            .is_err()
        );
    }
    assert!(
        Binding {
            parent_id: 0,
            token: "a".repeat(32)
        }
        .validate()
        .is_err()
    );
    let bytes = serde_json::to_vec(&operation(std::path::PathBuf::new())).unwrap();
    let encoded = bytes.iter().map(|b| format!("{b:02x}")).collect();
    assert!(Invocation::from_args(&["--oem-lan-policy".into(), encoded]).is_err());
    let reply = Reply {
        phase: Phase::Execute,
        sequence: 1,
        dirty: true,
        error: None,
    }
    .encoded()
    .unwrap();
    assert!(Reply::decode(&reply, Phase::Rollback, 1).is_err());
    assert!(Reply::decode(&reply, Phase::Execute, 2).is_err());
    let mut extra: serde_json::Value = serde_json::from_slice(&reply).unwrap();
    extra["old"] = true.into();
    assert!(Reply::decode(&serde_json::to_vec(&extra).unwrap(), Phase::Execute, 1).is_err());
    assert!(
        Reply::decode(
            &vec![b' '; protocol::MAX_REPLY_BYTES + 1],
            Phase::Execute,
            1
        )
        .is_err()
    );
}
