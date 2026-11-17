use super::*;
use crate::display::{Access, Display};
use anyhow::{Result, bail};
use std::sync::Mutex;

struct Memory(Mutex<Data>);
struct Data {
    displays: Vec<Display>,
    reads: usize,
    writes: Vec<String>,
    fail: Option<String>,
    ignore: bool,
}
impl Access for Memory {
    fn list(&self) -> Result<Vec<Display>> {
        let mut data = self.0.lock().unwrap();
        data.reads += 1;
        Ok(data.displays.clone())
    }
    fn test(&self, _: &Mode) -> Result<()> {
        Ok(())
    }
    fn set(&self, mode: &Mode) -> Result<()> {
        let mut data = self.0.lock().unwrap();
        data.writes.push(mode.device.clone());
        if data.fail.as_deref() == Some(&mode.device) {
            bail!("injected");
        }
        if !data.ignore {
            data.displays
                .iter_mut()
                .find(|display| display.current.device == mode.device)
                .unwrap()
                .current = mode.clone();
        }
        Ok(())
    }
}
fn plan() -> Plan {
    let changes = (1..=2)
        .map(|index| {
            let before = Mode {
                device: format!("DISPLAY{index}"),
                identity: format!("panel-{index}"),
                name: format!("Panel {index}"),
                width: 1920,
                height: 1080,
                bits: 32,
                orientation: 0,
                x: (index - 1) * 1920,
                y: 0,
                flags: 0,
                frequency: 60,
            };
            let mut target = before.clone();
            target.frequency = 144;
            Change { before, target }
        })
        .collect();
    Plan {
        changes,
        timeout_ms: 15_000,
    }
}
fn memory(plan: &Plan) -> Memory {
    Memory(Mutex::new(Data {
        displays: plan
            .targets()
            .into_iter()
            .map(|current| Display {
                current,
                frequencies: vec![60, 144],
                primary: true,
            })
            .collect(),
        reads: 0,
        writes: vec![],
        fail: None,
        ignore: false,
    }))
}

#[test]
fn batch_plan_rejects_duplicate_geometry_empty_and_legacy_shape() {
    let valid = plan();
    valid.validate().unwrap();
    let mut duplicate = valid.clone();
    duplicate.changes[1] = duplicate.changes[0].clone();
    assert!(duplicate.validate().is_err());
    let mut geometry = valid.clone();
    geometry.changes[1].target.x += 1;
    assert!(geometry.validate().is_err());
    let mut empty = valid.clone();
    empty.changes.clear();
    assert!(empty.validate().is_err());
    let mut deadline = valid.clone();
    deadline.timeout_ms = 15_001;
    assert!(deadline.validate().is_err());
    let old = serde_json::json!({ "before": valid.changes[0].before,
        "target": valid.changes[0].target, "timeoutMs": 15_000 });
    assert!(serde_json::from_value::<Plan>(old).is_err());
}

#[test]
fn guardian_accepts_only_exact_single_oem_rotation() {
    let mut valid = plan();
    valid.changes.truncate(1);
    valid.changes[0].target = valid.changes[0].before.oem_rotation().unwrap();
    valid.validate().unwrap();
    let mut invalid = valid.clone();
    invalid.changes[0].target.frequency = 144;
    assert!(invalid.validate().is_err());
    let mut invalid = valid.clone();
    invalid.changes[0].target.x += 1;
    assert!(invalid.validate().is_err());
    let mut invalid = valid.clone();
    invalid.changes.push(plan().changes[1].clone());
    assert!(invalid.validate().is_err());
    let mut portrait = valid.clone();
    portrait.changes[0].before.orientation = 1;
    portrait.changes[0].target = portrait.changes[0].before.oem_rotation().unwrap();
    portrait.validate().unwrap();
    portrait.changes[0].target.width = portrait.changes[0].before.width;
    assert!(portrait.validate().is_err());
}

#[test]
fn oversized_batch_and_invalid_identity_are_rejected() {
    let mut invalid = plan();
    invalid.changes[0].before.identity.clear();
    invalid.changes[0].target.identity.clear();
    assert!(invalid.validate().is_err());
    let mut large = plan();
    let first = large.changes[0].clone();
    large.changes = (0..32)
        .map(|index| {
            let mut change = first.clone();
            for mode in [&mut change.before, &mut change.target] {
                mode.device = format!("DISPLAY{index}");
                mode.identity = "x".repeat(2048);
            }
            change
        })
        .collect();
    assert!(large.validate().is_err());
}

#[test]
fn recovery_skips_unattempted_display_and_rechecks_written_modes() {
    let plan = plan();
    let io = memory(&plan);
    io.0.lock().unwrap().displays[1].current = plan.changes[1].before.clone();
    recovery::restore(&io, &plan.changes).unwrap();
    let data = io.0.lock().unwrap();
    assert_eq!(data.writes, ["DISPLAY1"]);
    assert!(
        data.displays
            .iter()
            .all(|display| display.current.frequency == 60)
    );
}

#[test]
fn failed_display_does_not_block_other_recovery_and_retry_skips_recovered() {
    let plan = plan();
    let io = memory(&plan);
    io.0.lock().unwrap().fail = Some("DISPLAY2".into());
    assert!(recovery::restore(&io, &plan.changes).is_err());
    assert_eq!(io.0.lock().unwrap().writes, ["DISPLAY2", "DISPLAY1"]);
    io.0.lock().unwrap().fail = None;
    recovery::restore(&io, &plan.changes).unwrap();
    assert_eq!(
        io.0.lock().unwrap().writes,
        ["DISPLAY2", "DISPLAY1", "DISPLAY2"]
    );
}

#[test]
fn external_change_is_preserved_while_other_owned_target_is_restored() {
    let plan = plan();
    let io = memory(&plan);
    io.0.lock().unwrap().displays[1].current.frequency = 120;
    let error = recovery::restore(&io, &plan.changes)
        .unwrap_err()
        .to_string();
    assert!(error.contains("外部修改"));
    let data = io.0.lock().unwrap();
    assert_eq!(data.writes, ["DISPLAY1"]);
    assert_eq!(data.displays[1].current.frequency, 120);
}

#[test]
fn successful_return_without_actual_recovery_is_rejected() {
    let plan = plan();
    let io = memory(&plan);
    io.0.lock().unwrap().ignore = true;
    assert!(
        recovery::restore(&io, &plan.changes)
            .unwrap_err()
            .to_string()
            .contains("回读")
    );
    assert_eq!(io.0.lock().unwrap().writes, ["DISPLAY2", "DISPLAY1"]);
}

#[test]
fn keep_requires_complete_batch_and_checks_every_actual_target() {
    let plan = plan();
    let io = memory(&plan);
    let (send, receive) = std::sync::mpsc::channel();
    send.send(Input::Prepare { plan: plan.clone() }).unwrap();
    send.send(Input::Arm).unwrap();
    send.send(Input::Keep {
        expected: vec![plan.changes[0].target.clone()],
    })
    .unwrap();
    send.send(Input::Keep {
        expected: plan.targets(),
    })
    .unwrap();
    let mut outputs = vec![];
    protocol::run(receive, &io, |output| outputs.push(output)).unwrap();
    assert!(matches!(outputs[2], Output::Rejected { .. }));
    assert!(matches!(outputs[3], Output::Kept));
    assert!(io.0.lock().unwrap().writes.is_empty());
}

#[test]
fn dropping_unarmed_parent_never_reads_or_writes_displays() {
    let plan = plan();
    let io = memory(&plan);
    let (send, receive) = std::sync::mpsc::channel();
    send.send(Input::Prepare { plan }).unwrap();
    drop(send);
    protocol::run(receive, &io, |_| {}).unwrap();
    let data = io.0.lock().unwrap();
    assert_eq!(data.reads, 0);
    assert!(data.writes.is_empty());
}

#[test]
fn armed_parent_disconnect_recovers_whole_batch() {
    let plan = plan();
    let io = memory(&plan);
    let (send, receive) = std::sync::mpsc::channel();
    send.send(Input::Prepare { plan: plan.clone() }).unwrap();
    send.send(Input::Arm).unwrap();
    drop(send);
    let mut outputs = vec![];
    protocol::run(receive, &io, |output| outputs.push(output)).unwrap();
    assert!(matches!(outputs.last().unwrap(), Output::Recovered));
    assert_eq!(io.0.lock().unwrap().writes, ["DISPLAY2", "DISPLAY1"]);
}
