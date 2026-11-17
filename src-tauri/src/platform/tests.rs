use super::*;
use std::cell::{Cell, RefCell};

struct Memory {
    flags: RefCell<[u8; 25]>,
    level: Cell<u8>,
    maximum: Cell<u8>,
    table: RefCell<[u8; 20]>,
    commands: RefCell<Vec<u8>>,
    fail_once: Cell<bool>,
    fail_all: Cell<bool>,
    ignore_once: Cell<bool>,
    swap: Cell<Option<(u8, u8)>>,
    deep: Cell<u8>,
    pd_reads: Cell<u32>,
}

#[test]
fn modern_standby_display_policy_uses_only_declared_deep_s5_state() {
    use crate::hal::power_events::DisplayPolicy;
    let io = Memory::new();
    assert_eq!(display_policy(&io).unwrap(), DisplayPolicy::DeepS5Enabled);
    io.deep.set(0);
    assert_eq!(display_policy(&io).unwrap(), DisplayPolicy::Allowed);
    io.deep.set(2);
    assert!(display_policy(&io).is_err());
    io.flags.borrow_mut()[24] = 0;
    assert_eq!(display_policy(&io).unwrap(), DisplayPolicy::Allowed);
    assert!(io.commands.borrow().is_empty());
}
impl Memory {
    fn new() -> Self {
        let mut flags = [0; 25];
        flags[0] = 1;
        flags[23] = 2;
        flags[24] = 1;
        let mut table = [0; 20];
        table[0] = 1;
        table[2] = 100;
        table[16..20].copy_from_slice(&[1, 3, 7, 15]);
        Self {
            flags: RefCell::new(flags),
            level: Cell::new(1),
            maximum: Cell::new(4),
            table: RefCell::new(table),
            commands: Default::default(),
            fail_once: Cell::new(false),
            fail_all: Cell::new(false),
            ignore_once: Cell::new(false),
            swap: Cell::new(None),
            deep: Cell::new(1),
            pd_reads: Cell::new(0),
        }
    }
    fn request(&self, level: u8) -> Request {
        Request {
            expected_revision: capture(self).unwrap().revision(),
            level,
        }
    }
}
impl Io for Memory {
    fn capabilities(&self) -> Result<Capabilities> {
        protocol::capabilities(self.flags.borrow().as_slice())
    }
    fn pd_data(&self) -> Result<([u8; 2], [u8; 20])> {
        self.pd_reads.set(self.pd_reads.get() + 1);
        Ok(([self.level.get(), self.maximum.get()], *self.table.borrow()))
    }
    fn set_pd(&self, level: u8) -> Result<()> {
        self.commands.borrow_mut().push(level);
        if !self.ignore_once.replace(false) {
            self.level.set(level);
        }
        if let Some((power, bits)) = self.swap.take() {
            let mut table = self.table.borrow_mut();
            table[2] = power;
            table[16] = bits;
        }
        if self.fail_once.replace(false) || self.fail_all.get() {
            bail!("隔离PD写入失败");
        }
        Ok(())
    }
    fn deep_s5(&self) -> Result<bool> {
        match self.deep.get() {
            0 => Ok(false),
            1 => Ok(true),
            _ => bail!("隔离Deep S5未知状态"),
        }
    }
}
#[test]
fn old_numbering_round_trips_raw_levels_without_ui_offset_aliases() {
    for (maximum, current, levels) in [
        (0, 0, vec![0, 1]),
        (0, 1, vec![0, 1]),
        (3, 1, vec![1, 2, 3]),
        (3, 3, vec![1, 2, 3]),
    ] {
        let snapshot = Snapshot::decode([current, maximum], [0; 20]).unwrap();
        assert_eq!(snapshot.levels, levels);
        assert_eq!(snapshot.zero_based(), maximum == 0);
        assert_eq!(snapshot.adapter_watts(), None);
        assert_eq!(snapshot.target(current).unwrap(), snapshot);
    }
}
#[test]
fn every_adapter_power_boundary_and_sparse_mask_uses_declared_bits_only() {
    for (power, levels) in [
        (0, vec![1]),
        (59, vec![1]),
        (60, vec![1, 2]),
        (89, vec![1, 2]),
        (90, vec![1, 2, 3]),
        (99, vec![1, 2, 3]),
        (100, vec![1, 2, 3, 4]),
        (255, vec![1, 2, 3, 4]),
    ] {
        let io = Memory::new();
        io.table.borrow_mut()[2] = power;
        let snapshot = capture(&io).unwrap();
        assert_eq!(snapshot.levels, levels);
        assert_eq!(snapshot.adapter_watts(), (power != 0).then_some(power));
    }
    let io = Memory::new();
    io.table.borrow_mut()[19] = 5;
    let snapshot = capture(&io).unwrap();
    assert_eq!(snapshot.levels, [1, 3]);
    assert!(snapshot.target(2).is_err());
}
#[test]
fn unknown_versions_masks_and_empty_candidates_never_send() {
    for case in 0..4 {
        let io = Memory::new();
        match case {
            0 => io.table.borrow_mut()[0] = 2,
            1 => io.table.borrow_mut()[19] = 0,
            2 => io.table.borrow_mut()[16] = 128,
            _ => io.flags.borrow_mut()[0] = 2,
        }
        assert!(capture(&io).is_err());
        assert!(io.commands.borrow().is_empty());
    }
    assert!(protocol::capabilities(&[0; 24]).is_err());
}
#[test]
fn no_capability_performs_no_pd_query_and_deep_s5_is_independent() {
    let io = Memory::new();
    io.flags.borrow_mut()[23] = 0;
    let profile = read(&io, &None, None).unwrap();
    assert!(profile.power_delivery.is_none());
    assert_eq!(profile.deep_s5, Some(true));
    assert_eq!(io.pd_reads.get(), 0);
    io.flags.borrow_mut()[23] = 2;
    io.deep.set(9);
    let profile = read(&io, &None, None).unwrap();
    assert!(profile.power_delivery.is_some());
    assert!(profile.deep_s5.is_none());
    assert_eq!(profile.warnings.len(), 1);
    io.flags.borrow_mut()[0] = 0;
    let profile = read(&io, &None, None).unwrap();
    assert!(profile.power_delivery.is_none() && profile.deep_s5.is_none());
}
#[test]
fn changed_revision_or_invalid_target_does_not_create_baseline() {
    let io = Memory::new();
    let old = io.request(2);
    io.level.set(2);
    let mut baseline = None;
    assert!(transaction::apply(&io, &mut baseline, &old).is_err());
    assert!(baseline.is_none());
    assert!(transaction::apply(&io, &mut baseline, &io.request(5)).is_err());
    assert!(io.commands.borrow().is_empty());
}
#[test]
fn writes_read_back_and_user_restore_keeps_the_first_raw_baseline() {
    let io = Memory::new();
    let mut baseline = None;
    transaction::apply(&io, &mut baseline, &io.request(2)).unwrap();
    transaction::apply(&io, &mut baseline, &io.request(3)).unwrap();
    assert_eq!(baseline.as_ref().unwrap().level(), 1);
    transaction::restore(&io, &baseline, &capture(&io).unwrap().revision()).unwrap();
    assert_eq!(*io.commands.borrow(), [2, 3, 1]);
    assert_eq!(io.level.get(), 1);
}
#[test]
fn partial_error_or_ignored_write_is_compensated_without_losing_baseline() {
    for partial in [true, false] {
        let io = Memory::new();
        let mut baseline = None;
        if partial {
            io.fail_once.set(true);
        } else {
            io.ignore_once.set(true);
        }
        assert!(transaction::apply(&io, &mut baseline, &io.request(3)).is_err());
        assert_eq!(io.level.get(), 1);
        assert!(baseline.is_some());
        assert_eq!(
            io.commands.borrow().as_slice(),
            if partial { &[3, 1][..] } else { &[3][..] }
        );
    }
}
#[test]
fn adapter_changes_can_recover_even_when_partial_target_is_now_outside_candidates() {
    let io = Memory::new();
    let mut baseline = None;
    io.swap.set(Some((45, 1)));
    assert!(transaction::apply(&io, &mut baseline, &io.request(4)).is_err());
    assert_eq!(*io.commands.borrow(), [4, 1]);
    assert_eq!(io.level.get(), 1);
}
#[test]
fn incompatible_adapter_blocks_restore_until_it_supports_original_level_again() {
    let io = Memory::new();
    io.level.set(4);
    let mut baseline = None;
    transaction::apply(&io, &mut baseline, &io.request(1)).unwrap();
    io.table.borrow_mut()[2] = 45;
    let revision = capture(&io).unwrap().revision();
    assert!(transaction::restore(&io, &baseline, &revision).is_err());
    assert_eq!(*io.commands.borrow(), [1]);
    assert!(
        read(&io, &baseline, None)
            .unwrap()
            .power_delivery
            .unwrap()
            .restore_blocked_reason
            .is_some()
    );
    io.table.borrow_mut()[2] = 100;
    transaction::restore(&io, &baseline, &capture(&io).unwrap().revision()).unwrap();
    assert_eq!(io.level.get(), 4);
}
#[test]
fn failed_compensation_is_reported_and_explicit_restore_can_retry() {
    let io = Memory::new();
    let mut baseline = None;
    io.fail_all.set(true);
    let error = transaction::apply(&io, &mut baseline, &io.request(3))
        .unwrap_err()
        .to_string();
    assert!(error.contains("恢复失败"));
    assert!(baseline.is_some());
    io.fail_all.set(false);
    io.level.set(3);
    transaction::restore(&io, &baseline, &capture(&io).unwrap().revision()).unwrap();
    assert_eq!(io.level.get(), 1);
}
#[test]
fn invalid_current_level_blocks_new_write_but_does_not_hide_known_facts() {
    let io = Memory::new();
    io.level.set(9);
    let profile = read(&io, &None, None).unwrap().power_delivery.unwrap();
    assert!(!profile.current_valid);
    assert!(profile.blocked_reason.is_none());
    assert!(transaction::apply(&io, &mut None, &io.request(1)).is_err());
    assert!(io.commands.borrow().is_empty());
}
#[test]
fn unchanged_selection_and_stale_restore_do_not_write() {
    let io = Memory::new();
    let mut baseline = None;
    transaction::apply(&io, &mut baseline, &io.request(1)).unwrap();
    assert!(baseline.is_none());
    assert!(io.commands.borrow().is_empty());
    transaction::apply(&io, &mut baseline, &io.request(2)).unwrap();
    assert!(transaction::restore(&io, &baseline, "old").is_err());
    assert_eq!(*io.commands.borrow(), [2]);
}
#[test]
fn reserved_payload_is_not_a_second_fact_source_and_contract_rejects_extra_fields() {
    let io = Memory::new();
    let before = capture(&io).unwrap();
    io.table.borrow_mut()[7] = 97;
    io.maximum.set(255);
    assert_eq!(capture(&io).unwrap(), before);
    let mut unused = [71; 20];
    unused[..2].fill(0);
    assert_eq!(
        Snapshot::decode([0, 0], unused).unwrap(),
        Snapshot::decode([0, 0], [0; 20]).unwrap()
    );
    assert!(
        serde_json::from_value::<Request>(
            serde_json::json!({"expectedRevision":"x","level":1,"watts":100})
        )
        .is_err()
    );
}

#[test]
fn encoding_change_blocks_further_writes_and_initial_restore() {
    let io = Memory::new();
    let mut baseline = None;
    transaction::apply(&io, &mut baseline, &io.request(2)).unwrap();
    io.table.borrow_mut().fill(0);
    io.maximum.set(0);
    io.level.set(1);
    let request = io.request(0);
    assert!(transaction::apply(&io, &mut baseline, &request).is_err());
    assert!(transaction::restore(&io, &baseline, &request.expected_revision).is_err());
    assert_eq!(*io.commands.borrow(), [2]);
    assert!(
        read(&io, &baseline, None)
            .unwrap()
            .power_delivery
            .unwrap()
            .blocked_reason
            .is_some()
    );
}

#[test]
fn calibration_interlock_rejects_pd_apply_and_restore_before_any_native_transport() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static CALLS: AtomicUsize = AtomicUsize::new(0);
    unsafe extern "system" fn buffer(_: i32, _: *mut u8) -> i32 {
        CALLS.fetch_add(1, Ordering::SeqCst);
        0
    }
    unsafe extern "system" fn settings(_: i32, _: i32, _: i32, _: *mut u8) -> i32 {
        CALLS.fetch_add(1, Ordering::SeqCst);
        0
    }
    unsafe extern "system" fn send(_: i32, _: *const u8, _: i32) -> i32 {
        CALLS.fetch_add(1, Ordering::SeqCst);
        0
    }
    unsafe extern "system" fn save(_: i32, _: i32, _: i32, _: *const u8) -> i32 {
        CALLS.fetch_add(1, Ordering::SeqCst);
        0
    }
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("calibration.json");
    std::fs::write(&path, b"{broken").unwrap();
    let mut hal = Hal::test_instance(Dchu::test_io(buffer, settings, send, save), None);
    hal.bind_calibration_record(path);
    let controller = Controller::default();
    assert!(
        controller
            .apply(
                &hal,
                &Request {
                    expected_revision: "unused".into(),
                    level: 2
                }
            )
            .unwrap_err()
            .to_string()
            .contains("校准")
    );
    assert!(
        controller
            .restore(&hal, "unused")
            .unwrap_err()
            .to_string()
            .contains("校准")
    );
    assert!(
        controller
            .run_pd_event(&hal, "unused", || Ok(()))
            .unwrap_err()
            .to_string()
            .contains("校准")
    );
    assert_eq!(CALLS.load(Ordering::SeqCst), 0);
}

#[test]
fn pd_event_shares_baseline_and_keeps_authorization_stable_across_level_changes() {
    let io = Memory::new();
    io.level.set(4);
    let mut baseline = None;
    let before = pd_event::preview(&io, &baseline, None).unwrap();
    let identity = before.identity.unwrap();
    assert!(pd_event::execute(&io, &mut baseline, &identity, &mut || Ok(())).unwrap());
    assert_eq!(io.level.get(), 1);
    assert_eq!(
        pd_event::preview(&io, &baseline, None)
            .unwrap()
            .identity
            .as_deref(),
        Some(identity.as_str())
    );
    assert!(!pd_event::execute(&io, &mut baseline, &identity, &mut || Ok(())).unwrap());
    transaction::apply(&io, &mut baseline, &io.request(3)).unwrap();
    transaction::restore(&io, &baseline, &capture(&io).unwrap().revision()).unwrap();
    assert_eq!(io.level.get(), 4);
    assert_eq!(*io.commands.borrow(), [1, 3, 4]);
}

#[test]
fn pd_event_binds_numbering_and_adapter_without_guessing_target() {
    let io = Memory::new();
    let modern = pd_event::preview(&io, &None, None)
        .unwrap()
        .identity
        .unwrap();
    io.table.borrow_mut().fill(0);
    io.maximum.set(0);
    io.level.set(0);
    let legacy = pd_event::preview(&io, &None, None)
        .unwrap()
        .identity
        .unwrap();
    assert_ne!(legacy, modern);
    assert!(pd_event::execute(&io, &mut None, &modern, &mut || Ok(())).is_err());
    assert!(pd_event::execute(&io, &mut None, &legacy, &mut || Ok(())).unwrap());
    assert_eq!(io.level.get(), 1);
    let io = Memory::new();
    io.table.borrow_mut()[19] = 2;
    io.level.set(2);
    assert!(
        pd_event::preview(&io, &None, None)
            .unwrap()
            .identity
            .is_none()
    );
    assert!(io.commands.borrow().is_empty());
}

#[test]
fn pd_event_revocation_or_adapter_change_during_permission_check_has_zero_output() {
    for change in [false, true] {
        let io = Memory::new();
        io.level.set(4);
        let identity = pd_event::preview(&io, &None, None)
            .unwrap()
            .identity
            .unwrap();
        let mut calls = 0;
        let mut baseline = None;
        let result = pd_event::execute(&io, &mut baseline, &identity, &mut || {
            calls += 1;
            if calls == 2 {
                if change {
                    io.table.borrow_mut()[2] = 90;
                } else {
                    bail!("许可撤销");
                }
            }
            Ok(())
        });
        assert!(result.is_err());
        assert!(baseline.is_none());
        assert!(io.commands.borrow().is_empty());
    }
}

#[test]
fn pd_event_failure_compensates_and_unrestorable_baseline_blocks_new_write() {
    let io = Memory::new();
    io.level.set(4);
    let identity = pd_event::preview(&io, &None, None)
        .unwrap()
        .identity
        .unwrap();
    let mut baseline = None;
    io.fail_once.set(true);
    assert!(pd_event::execute(&io, &mut baseline, &identity, &mut || Ok(())).is_err());
    assert_eq!(io.level.get(), 4);
    assert!(baseline.is_some());
    io.table.borrow_mut()[2] = 45;
    let preview = pd_event::preview(&io, &baseline, None).unwrap();
    assert!(
        preview.identity.is_none() && preview.current.unwrap().restore_blocked_reason.is_some()
    );
    assert_eq!(*io.commands.borrow(), [1, 4]);
}
