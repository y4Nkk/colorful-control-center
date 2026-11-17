use super::*;
use crate::battery::calibration::{
    store::{FileStore, Record, Store},
    worker::{self, Input, Output},
};
use std::{
    cell::{Cell, RefCell},
    io::Cursor,
    sync::mpsc,
};

const OP: &str = "1234567890abcdef1234567890abcdef";
struct Memory {
    record: RefCell<Option<Record>>,
    saves: Cell<usize>,
    fail_at: BTreeSet<usize>,
}
impl Memory {
    fn prepared() -> Self {
        Self {
            record: RefCell::new(Some(Record::prepared(OP.into(), baseline()).unwrap())),
            saves: Cell::new(0),
            fail_at: BTreeSet::new(),
        }
    }
    fn latest(&self) -> Record {
        self.record.borrow().clone().unwrap()
    }
}
impl Store for Memory {
    fn load(&self) -> Result<Option<Record>> {
        Ok(self.record.borrow().clone())
    }
    fn save(&self, record: &Record) -> Result<()> {
        record.validate()?;
        self.saves.set(self.saves.get() + 1);
        if self.fail_at.contains(&self.saves.get()) {
            bail!("fake atomic replace failure");
        }
        *self.record.borrow_mut() = Some(record.clone());
        Ok(())
    }
}
fn message(kind: &str) -> Input {
    serde_json::from_value(serde_json::json!({"event":kind,"operation":OP})).unwrap()
}
fn run_sequence(io: &mut Fake, store: &Memory, messages: Vec<Input>) -> Vec<Output> {
    let (sender, input) = mpsc::channel();
    for message in messages {
        sender.send(message).unwrap();
    }
    drop(sender);
    let mut output = Vec::new();
    worker::run(input, io, store, |m| {
        output.push(m);
        Ok(())
    })
    .unwrap();
    output
}
#[test]
fn unarmed_or_wrong_operation_never_touches_control() {
    for wrong in [false, true] {
        let mut io = Fake::default();
        let store = Memory::prepared();
        let mut messages = vec![message("prepare")];
        if wrong {
            messages.push(Input::Arm {
                operation: "other".into(),
            });
        }
        let output = run_sequence(&mut io, &store, messages);
        assert!(matches!(output.as_slice(), [Output::Ready]));
        assert!(io.words.is_empty());
        assert_eq!(store.saves.get(), 0);
        assert!(store.latest().pending);
    }
}
#[test]
fn eof_after_arm_releases_then_persists_failed_result() {
    let mut io = Fake::default();
    let store = Memory::prepared();
    let output = run_sequence(&mut io, &store, vec![message("prepare"), message("arm")]);
    assert_eq!(io.words.len(), 7);
    assert_eq!(io.restored, 1);
    assert!(!io.awake);
    assert!(!store.latest().pending);
    assert_eq!(store.latest().state.unwrap().status, Status::Failed);
    assert!(matches!(output.last(), Some(Output::Finished)));
}
#[test]
fn explicit_stop_and_invalid_request_both_restore_with_distinct_results() {
    for valid in [true, false] {
        let mut io = Fake::default();
        let store = Memory::prepared();
        let request = if valid {
            message("stop")
        } else {
            Input::Stop {
                operation: "old-operation".into(),
            }
        };
        run_sequence(
            &mut io,
            &store,
            vec![message("prepare"), message("arm"), request],
        );
        assert_eq!(
            store.latest().outcome,
            Some(if valid {
                Status::Stopped
            } else {
                Status::Failed
            })
        );
        assert!(!store.latest().pending);
        assert_eq!(io.restored, 1);
    }
}
#[test]
fn revision_change_between_ready_and_arm_rejects_without_control_writes() {
    let mut io = Fake::default();
    io.changed_on_verification = Some(2);
    let store = Memory::prepared();
    let output = run_sequence(&mut io, &store, vec![message("prepare"), message("arm")]);
    assert!(io.words.is_empty());
    assert!(!store.latest().pending);
    assert_eq!(store.latest().outcome, Some(Status::Failed));
    assert!(matches!(output.last(), Some(Output::Rejected { .. })));
}
#[test]
fn first_progress_save_failure_cancels_and_restores_before_next_stage() {
    let mut io = Fake::default();
    let mut store = Memory::prepared();
    store.fail_at.insert(3);
    run_sequence(&mut io, &store, vec![message("prepare"), message("arm")]);
    assert_eq!(io.words.len(), 7);
    assert_eq!(io.restored, 1);
    let latest = store.latest();
    assert_eq!(latest.outcome, Some(Status::Failed));
    assert!(!latest.pending);
    assert!(latest.error.unwrap().contains("保存失败"));
}

#[test]
fn failed_arm_commit_or_previously_armed_record_never_starts_a_task() {
    for previous_arm in [false, true] {
        let mut io = Fake::default();
        let mut store = Memory::prepared();
        if previous_arm {
            store.record.borrow_mut().as_mut().unwrap().armed = true;
        } else {
            store.fail_at.insert(1);
        }
        let (sender, input) = mpsc::channel();
        sender.send(message("prepare")).unwrap();
        sender.send(message("arm")).unwrap();
        drop(sender);
        assert!(worker::run(input, &mut io, &store, |_| Ok(())).is_err());
        assert!(io.words.is_empty());
        assert_eq!(io.restored, 0);
        assert_eq!(store.latest().armed, previous_arm);
    }
}
#[test]
fn blocked_status_output_cancels_control_even_if_parent_input_stays_open() {
    let mut io = Fake::default();
    let store = Memory::prepared();
    let (sender, input) = mpsc::channel();
    sender.send(message("prepare")).unwrap();
    sender.send(message("arm")).unwrap();
    worker::run(input, &mut io, &store, |m| {
        if matches!(m, Output::State { .. }) {
            bail!("fake blocked output");
        }
        Ok(())
    })
    .unwrap();
    assert_eq!(io.restored, 1);
    assert!(!store.latest().pending);
    assert!(store.latest().error.unwrap().contains("状态管道断开"));
}
#[test]
fn live_pipe_without_heartbeat_times_out_and_restores() {
    let mut io = Fake::default();
    let store = Memory::prepared();
    let (sender, input) = mpsc::channel();
    sender.send(message("prepare")).unwrap();
    sender.send(message("arm")).unwrap();
    worker::run(input, &mut io, &store, |_| Ok(())).unwrap();
    assert_eq!(io.restored, 1);
    assert!(!store.latest().pending);
    assert!(store.latest().error.unwrap().contains("心跳停止"));
    drop(sender);
}
#[test]
fn protocol_rejects_unknown_fields_malformed_and_oversized_lines() {
    for bytes in [
        br#"{"event":"prepare","operation":"x","legacy":true}"#.to_vec(),
        b"not-json\n".to_vec(),
        vec![b'x'; 65_537],
    ] {
        let input = worker::read(Cursor::new(bytes));
        let mut io = Fake::default();
        assert!(worker::run(input, &mut io, &Memory::prepared(), |_| Ok(())).is_err());
        assert!(io.words.is_empty());
    }
}
#[test]
fn prepared_file_is_atomic_strict_and_corruption_is_preserved() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("battery-calibration.json");
    let store = FileStore(path.clone());
    assert!(store.load().unwrap().is_none());
    let record = Record::prepared(OP.into(), baseline()).unwrap();
    store.save(&record).unwrap();
    assert_eq!(store.load().unwrap().unwrap().operation, OP);
    let bytes = std::fs::read(&path).unwrap();
    let mut raw: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    raw["version"] = serde_json::json!(2);
    let corrupt = serde_json::to_vec(&raw).unwrap();
    std::fs::write(&path, &corrupt).unwrap();
    assert!(store.load().is_err());
    assert_eq!(std::fs::read(&path).unwrap(), corrupt);
}
#[test]
fn inconsistent_restore_records_are_rejected() {
    for kind in 0..5 {
        let mut record = Record::prepared(OP.into(), baseline()).unwrap();
        match kind {
            0 => record.pending = false,
            1 => record.operation = "not-an-operation".into(),
            2 => record.outcome = Some(Status::Running),
            3 => record.outcome = Some(Status::Completed),
            _ => record.baseline.charge.start_values.clear(),
        }
        assert!(record.validate().is_err(), "kind {kind}");
    }
}
#[test]
fn unrelated_source_with_no_insertion_strings_does_not_break_subscription() {
    let mut raw = record(1, "other", "ignored");
    raw[26..28].fill(0);
    raw[36..40].fill(0);
    assert_eq!(events::parse(&raw, 1).unwrap(), (2, vec![]));
}

#[test]
fn pending_or_corrupt_calibration_records_block_shared_charge_writes() {
    let directory = tempfile::tempdir().unwrap();
    let energy_path = directory.path().join("energy-policy.json");
    let path = directory.path().join("battery-calibration.json");
    assert!(
        crate::battery::calibration::charge_lock(&energy_path)
            .unwrap()
            .is_none()
    );
    FileStore(path.clone())
        .save(&Record::prepared(OP.into(), baseline()).unwrap())
        .unwrap();
    assert!(crate::battery::calibration::ensure_system_plan_available(&path).is_err());
    assert!(
        crate::battery::calibration::charge_lock(&energy_path)
            .unwrap()
            .is_some()
    );
    std::fs::write(&path, b"not-json").unwrap();
    assert!(crate::battery::calibration::charge_lock(&energy_path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"not-json");
}

#[test]
fn plan_intent_commit_precedes_import_and_failed_commit_does_not_delete_any_plan() {
    let mut io = Fake::default();
    let mut store = Memory::prepared();
    store.fail_at.insert(2);
    run_sequence(&mut io, &store, vec![message("prepare"), message("arm")]);
    assert_eq!(io.plan_entered, 0);
    assert_eq!(io.plan_restored, 0);
    assert!(!store.latest().plan_prepared);
    assert!(!store.latest().pending);
}

#[test]
fn occupied_plan_partial_import_external_change_and_recovery_failure_have_distinct_effects() {
    for failure in ["prepare", "import", "changed", "restore"] {
        let mut io = Fake::default();
        io.plan_fault = Some(failure);
        let mut task = start(&mut io);
        if failure == "changed" {
            task.tick(&mut io, Duration::from_secs(1));
        }
        if failure == "restore" {
            task.stop(&mut io);
        }
        if failure == "prepare" {
            assert_eq!(io.plan_entered, 0);
            assert_eq!(io.plan_restored, 0);
        } else {
            assert!(io.plan_restored > 0);
        }
        if failure == "restore" {
            assert_eq!(task.state.status, Status::RecoveryRequired);
            io.plan_fault = None;
            task.retry_restore(&mut io);
            assert_eq!(task.state.status, Status::Stopped);
        } else {
            assert_eq!(task.state.status, Status::Failed);
        }
    }
}

#[test]
fn parent_recovery_validates_the_record_and_skips_unarmed_control_writes() {
    for armed in [false, true] {
        let mut io = Fake::default();
        let store = Memory::prepared();
        let mut record = store.latest();
        if armed {
            let task = start(&mut io);
            record.update(&task);
            record.set_error("fixture child exited".into());
        }
        crate::battery::calibration::store::recover(&mut io, &store, record).unwrap();
        assert!(!store.latest().pending);
        if armed {
            assert_eq!(io.restored, 1);
            assert_eq!(io.plan_restored, 1);
            assert_eq!(store.latest().state.unwrap().status, Status::Failed);
        } else {
            assert!(io.words.is_empty());
            assert_eq!(io.plan_restored, 0);
        }
    }
}
