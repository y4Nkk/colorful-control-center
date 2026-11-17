use super::*;

struct Memory {
    state: Mutex<Snapshot>,
    writes: Mutex<Vec<String>>,
    fail: Mutex<Vec<usize>>,
    add_on_write: Mutex<bool>,
    blocked: Mutex<bool>,
    ignore_set: Mutex<bool>,
    ignore_save: Mutex<bool>,
}
impl Memory {
    fn new() -> Self {
        Self {
            state: Mutex::new(Snapshot {
                endpoints: vec![
                    Endpoint {
                        id: "A".into(),
                        name: Some("内置麦克风".into()),
                        muted: false,
                    },
                    Endpoint {
                        id: "B".into(),
                        name: Some("USB麦克风".into()),
                        muted: true,
                    },
                    Endpoint {
                        id: "C".into(),
                        name: None,
                        muted: false,
                    },
                ],
                saved: 0,
            }),
            writes: Mutex::new(vec![]),
            fail: Mutex::new(vec![]),
            add_on_write: Mutex::new(false),
            blocked: Mutex::new(false),
            ignore_set: Mutex::new(false),
            ignore_save: Mutex::new(false),
        }
    }
    fn result(&self) -> Result<()> {
        let count = self.writes.lock().unwrap().len();
        if self.fail.lock().unwrap().contains(&count) {
            bail!("隔离部分写入失败");
        }
        Ok(())
    }
    fn plan(&self, initial: &Option<Snapshot>, muted: bool) -> Plan {
        let before = transaction::capture(self).unwrap();
        let mut endpoints = before.endpoints.clone();
        for endpoint in &mut endpoints {
            endpoint.muted = muted;
        }
        let after = before
            .target(&Target {
                endpoints,
                saved_muted: muted,
            })
            .unwrap();
        transaction::prepare(before, after, initial).unwrap()
    }
}
impl Io for Memory {
    fn snapshot(&self) -> Result<Snapshot> {
        Ok(self.state.lock().unwrap().clone())
    }
    fn ensure_available(&self) -> Result<()> {
        if *self.blocked.lock().unwrap() {
            bail!("隔离原厂控制者仍在运行");
        }
        Ok(())
    }
    fn set_muted(&self, id: &str, muted: bool) -> Result<()> {
        self.writes.lock().unwrap().push(format!("{id}:{muted}"));
        let mut state = self.state.lock().unwrap();
        if !*self.ignore_set.lock().unwrap() {
            state
                .endpoints
                .iter_mut()
                .find(|e| e.id == id)
                .unwrap()
                .muted = muted;
        }
        if std::mem::take(&mut *self.add_on_write.lock().unwrap()) {
            state.endpoints.push(Endpoint {
                id: "D".into(),
                name: None,
                muted: false,
            });
        }
        drop(state);
        self.result()
    }
    fn save(&self, value: u8) -> Result<()> {
        self.writes.lock().unwrap().push(format!("saved:{value}"));
        if !*self.ignore_save.lock().unwrap() {
            self.state.lock().unwrap().saved = value;
        }
        self.result()
    }
}
#[test]
fn mixed_endpoints_share_one_target_and_exact_first_baseline() {
    let io = Memory::new();
    let before = io.snapshot().unwrap();
    let mut initial = None;
    let first = io.plan(&initial, true);
    transaction::execute(&io, &mut initial, &first, &mut false).unwrap();
    assert_eq!(initial.as_ref(), Some(&before));
    assert_eq!(*io.writes.lock().unwrap(), ["A:true", "C:true", "saved:1"]);
    let second = io.plan(&initial, false);
    transaction::execute(&io, &mut initial, &second, &mut false).unwrap();
    assert_eq!(initial.as_ref(), Some(&before));
    transaction::rollback(&io, &second).unwrap();
    transaction::rollback(&io, &first).unwrap();
    assert_eq!(io.snapshot().unwrap(), before);
}
#[test]
fn every_endpoint_and_preference_failure_restores_only_attempted_state() {
    for failed in 1..=3 {
        let io = Memory::new();
        let before = io.snapshot().unwrap();
        io.fail.lock().unwrap().push(failed);
        let mut initial = None;
        let plan = io.plan(&initial, true);
        let mut pending = false;
        assert!(transaction::execute(&io, &mut initial, &plan, &mut pending).is_err());
        assert!(!pending);
        assert_eq!(io.snapshot().unwrap(), before);
        assert_eq!(initial, Some(before));
    }
}
#[test]
fn failed_compensation_retry_preserves_unattempted_external_endpoint() {
    let io = Memory::new();
    *io.fail.lock().unwrap() = vec![1, 2];
    let plan = io.plan(&None, true);
    let mut initial = None;
    let mut pending = false;
    assert!(transaction::execute(&io, &mut initial, &plan, &mut pending).is_err());
    assert!(pending && initial.is_some());
    io.state.lock().unwrap().endpoints[1].muted = false;
    io.fail.lock().unwrap().clear();
    transaction::rollback(&io, &plan).unwrap();
    assert!(!io.state.lock().unwrap().endpoints[1].muted);
    assert!(
        io.writes
            .lock()
            .unwrap()
            .iter()
            .all(|write| write.starts_with("A:"))
    );
}
#[test]
fn device_appearance_is_not_taken_over_and_current_attempt_is_recovered() {
    let io = Memory::new();
    let plan = io.plan(&None, true);
    *io.add_on_write.lock().unwrap() = true;
    assert!(transaction::execute(&io, &mut None, &plan, &mut false).is_err());
    let current = io.snapshot().unwrap();
    assert!(!current.endpoints[0].muted && !current.endpoints[3].muted);
    assert_eq!(*io.writes.lock().unwrap(), ["A:true", "A:false"]);
}
#[test]
fn stale_snapshot_owner_conflict_and_changed_baseline_never_write() {
    let io = Memory::new();
    let plan = io.plan(&None, true);
    io.state.lock().unwrap().saved = 1;
    assert!(transaction::execute(&io, &mut None, &plan, &mut false).is_err());
    let plan = io.plan(&None, true);
    *io.blocked.lock().unwrap() = true;
    assert!(transaction::execute(&io, &mut None, &plan, &mut false).is_err());
    let mut initial = io.snapshot().unwrap();
    initial.endpoints.pop();
    assert!(
        transaction::prepare(plan.before.clone(), plan.before.clone(), &Some(initial)).is_err()
    );
    assert!(io.writes.lock().unwrap().is_empty());
}
#[test]
fn unknown_original_preference_is_restored_exactly_without_defaulting() {
    let io = Memory::new();
    io.state.lock().unwrap().saved = 77;
    let before = io.snapshot().unwrap();
    let plan = io.plan(&None, true);
    transaction::execute(&io, &mut None, &plan, &mut false).unwrap();
    transaction::rollback(&io, &plan).unwrap();
    assert_eq!(io.snapshot().unwrap(), before);
}
#[test]
fn external_saved_preference_is_not_overwritten_by_later_rollback() {
    let io = Memory::new();
    let plan = io.plan(&None, true);
    transaction::execute(&io, &mut None, &plan, &mut false).unwrap();
    io.state.lock().unwrap().saved = 99;
    assert!(
        transaction::rollback(&io, &plan)
            .unwrap_err()
            .to_string()
            .contains("外部修改")
    );
    assert_eq!(io.state.lock().unwrap().saved, 99);
    io.state.lock().unwrap().saved = 1;
    transaction::rollback(&io, &plan).unwrap();
    assert_eq!(io.state.lock().unwrap().saved, 0);
}
#[test]
fn empty_duplicate_unsorted_and_legacy_targets_are_rejected() {
    let io = Memory::new();
    let before = io.snapshot().unwrap();
    let mut target = Target {
        endpoints: before.endpoints,
        saved_muted: false,
    };
    target.endpoints.reverse();
    assert!(target.validate().is_err());
    target.endpoints = vec![target.endpoints[0].clone(); 2];
    assert!(target.validate().is_err());
    target.endpoints.clear();
    assert!(target.validate().is_err());
    assert!(
        serde_json::from_str::<Request>(r#"{"expectedRevision":"x","muted":true,"volume":50}"#)
            .is_err()
    );
    assert!(
        serde_json::from_str::<Target>(r#"{"endpoints":[],"savedMuted":true,"legacy":0}"#).is_err()
    );
    assert!(io.writes.lock().unwrap().is_empty());
}

#[test]
fn successful_but_ignored_endpoint_or_preference_writes_fail_readback_and_recover() {
    for ignored_set in [true, false] {
        let io = Memory::new();
        let before = io.snapshot().unwrap();
        *io.ignore_set.lock().unwrap() = ignored_set;
        *io.ignore_save.lock().unwrap() = !ignored_set;
        let plan = io.plan(&None, true);
        let mut initial = None;
        let mut pending = false;
        let error = transaction::execute(&io, &mut initial, &plan, &mut pending).unwrap_err();
        assert!(error.to_string().contains("回读不一致"));
        assert!(!pending && initial == Some(before.clone()));
        assert_eq!(io.snapshot().unwrap(), before);
        assert!(
            io.writes
                .lock()
                .unwrap()
                .iter()
                .all(|write| !write.starts_with("B:"))
        );
    }
}
