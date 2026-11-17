use super::*;
use std::sync::{Arc, Mutex};
#[path = "profile_tests.rs"]
mod profiles;
#[derive(Clone)]
struct Fake(Arc<Mutex<Data>>);
struct Data {
    modes: Vec<Display>,
    writes: usize,
    fail: Vec<usize>,
    test_fail: bool,
}
fn mode() -> Mode {
    Mode {
        device: "DISPLAY1".into(),
        identity: "panel-a".into(),
        name: "Panel".into(),
        width: 1920,
        height: 1080,
        bits: 32,
        orientation: 0,
        x: 0,
        y: 0,
        flags: 0,
        frequency: 60,
    }
}
fn setup() -> (Controller, Fake) {
    let io = Fake(Arc::new(Mutex::new(Data {
        modes: vec![Display {
            current: mode(),
            frequencies: vec![60, 144],
            primary: true,
        }],
        writes: 0,
        fail: vec![],
        test_fail: false,
    })));
    (
        Controller::with_guards(io.clone(), guardian::MemoryFactory),
        io,
    )
}
impl Access for Fake {
    fn list(&self) -> Result<Vec<Display>> {
        Ok(self.0.lock().unwrap().modes.clone())
    }
    fn test(&self, _: &Mode) -> Result<()> {
        if self.0.lock().unwrap().test_fail {
            bail!("TEST rejected");
        }
        Ok(())
    }
    fn set(&self, mode: &Mode) -> Result<()> {
        let mut d = self.0.lock().unwrap();
        d.writes += 1;
        let n = d.writes;
        let target = d
            .modes
            .iter_mut()
            .find(|d| d.current.device == mode.device)
            .ok_or_else(|| anyhow!("missing"))?;
        if target.current.identity != mode.identity {
            bail!("identity changed");
        }
        target.current = mode.clone();
        if d.fail.contains(&n) {
            bail!("write {n}");
        }
        Ok(())
    }
}
#[test]
fn stale_unsupported_and_driver_test_reject_before_writes() {
    let (c, io) = setup();
    let mut old = mode();
    old.frequency = 120;
    assert!(c.apply(&old, 144, 0).is_err());
    assert!(c.apply(&mode(), 120, 0).is_err());
    io.0.lock().unwrap().test_fail = true;
    assert!(c.apply(&mode(), 144, 0).is_err());
    assert_eq!(io.0.lock().unwrap().writes, 0);
    assert!(!c.read().unwrap().can_restore);
}
#[test]
fn trial_requires_keep_and_timeout_restores_previous_without_losing_initial() {
    let (c, io) = setup();
    c.apply(&mode(), 144, 100).unwrap();
    let first = c.read().unwrap().displays[0].current.clone();
    assert_eq!(first.frequency, 144);
    c.keep(1, &first, 200).unwrap();
    c.apply(&first, 60, 300).unwrap();
    c.tick(15_299).unwrap();
    assert_eq!(io.list().unwrap()[0].current.frequency, 60);
    c.tick(15_300).unwrap();
    assert_eq!(io.list().unwrap()[0].current.frequency, 144);
    c.restore().unwrap();
    assert_eq!(io.list().unwrap()[0].current, mode());
}
#[test]
fn expired_or_unreviewed_keep_is_rejected() {
    let (c, _) = setup();
    c.apply(&mode(), 144, 0).unwrap();
    let target = c.read().unwrap().displays[0].current.clone();
    assert!(c.keep(1, &mode(), 1).is_err());
    assert!(c.keep(1, &target, 15_000).is_err());
    c.tick(15_000).unwrap();
    assert!(c.read().unwrap().pending.is_none());
}
#[test]
fn partial_failure_retains_snapshot_and_timeout_retries_recovery() {
    let (c, io) = setup();
    io.0.lock().unwrap().fail = vec![1, 2];
    assert!(
        c.apply(&mode(), 144, 0)
            .unwrap_err()
            .to_string()
            .contains("恢复失败")
    );
    assert!(c.read().unwrap().can_restore);
    assert!(c.read().unwrap().pending.is_some());
    c.tick(15_000).unwrap();
    assert!(c.read().unwrap().pending.is_none());
    assert_eq!(io.list().unwrap()[0].current, mode());
}
#[test]
fn replacement_display_never_receives_original_snapshot() {
    let (c, io) = setup();
    c.apply(&mode(), 144, 0).unwrap();
    io.0.lock().unwrap().modes[0].current.identity = "other-panel".into();
    assert!(c.restore().is_err());
    assert!(c.read().unwrap().can_restore);
}
#[derive(Clone, Copy)]
enum GuardFault {
    Start,
    ExpiredDuringWrite,
    ExitAfterWrite,
}
struct FaultFactory(GuardFault);
struct FaultMonitor(GuardFault, usize);
impl guardian::Factory for FaultFactory {
    fn start(&self, _: guardian::Plan) -> Result<Box<dyn guardian::Monitor>> {
        if matches!(self.0, GuardFault::Start) {
            bail!("监护启动失败");
        }
        Ok(Box::new(FaultMonitor(self.0, 0)))
    }
}
impl guardian::Monitor for FaultMonitor {
    fn keep(&mut self, _: &[Mode]) -> Result<()> {
        Ok(())
    }
    fn restored(&mut self, _: &[Mode]) -> Result<()> {
        Ok(())
    }
    fn recovered(&mut self) -> Result<bool> {
        self.1 += 1;
        if matches!(self.0, GuardFault::ExitAfterWrite) && self.1 > 1 {
            bail!("监护进程已退出");
        }
        Ok(matches!(self.0, GuardFault::ExpiredDuringWrite))
    }
}
#[test]
fn guardian_start_failure_prevents_target_write_and_baseline() {
    let (_, io) = setup();
    let c = Controller::with_guards(io.clone(), FaultFactory(GuardFault::Start));
    assert!(c.apply(&mode(), 144, 0).is_err());
    assert_eq!(io.0.lock().unwrap().writes, 0);
    assert!(!c.read().unwrap().can_restore);
}
#[test]
fn expiry_during_write_compensates_target_and_preserves_initial() {
    let (_, io) = setup();
    let c = Controller::with_guards(io.clone(), FaultFactory(GuardFault::ExpiredDuringWrite));
    assert!(c.apply(&mode(), 144, 0).is_err());
    assert_eq!(io.0.lock().unwrap().writes, 2);
    assert_eq!(io.list().unwrap()[0].current, mode());
    assert!(c.read().unwrap().can_restore);
    assert!(c.read().unwrap().pending.is_none());
}
#[test]
fn unexpected_child_exit_triggers_recovery_before_deadline() {
    let (_, io) = setup();
    let c = Controller::with_guards(io.clone(), FaultFactory(GuardFault::ExitAfterWrite));
    c.apply(&mode(), 144, 0).unwrap();
    c.tick(1).unwrap();
    let profile = c.read().unwrap();
    assert_eq!(io.list().unwrap()[0].current, mode());
    assert!(profile.pending.is_none());
    assert!(profile.error.unwrap().contains("监护进程已退出"));
}
#[test]
fn display_readback_ignores_label_but_keeps_identity_and_geometry() {
    let before = mode();
    let mut readback = before.clone();
    readback.name = "Driver renamed this output".into();
    assert!(before.same_state(&readback));
    readback.identity = "other-panel".into();
    assert!(!before.same_state(&readback));
    readback.identity = before.identity.clone();
    readback.frequency = 144;
    assert!(!before.same_state(&readback));
}

#[test]
fn oem_rotation_matches_all_four_original_direction_branches() {
    for (before, after, swap) in [(0, 2, false), (2, 0, false), (1, 2, true), (3, 2, true)] {
        let mut source = mode();
        source.orientation = before;
        let target = source.oem_rotation().unwrap();
        assert_eq!(target.orientation, after);
        assert_eq!(
            (target.width, target.height),
            if swap { (1080, 1920) } else { (1920, 1080) }
        );
        assert_eq!(target.frequency, source.frequency);
    }
    let mut invalid = mode();
    invalid.orientation = 4;
    assert!(invalid.oem_rotation().is_err());
}

#[test]
fn rotation_trial_reuses_guard_timeout_and_original_baseline() {
    let (controller, io) = setup();
    let preview = controller.read().unwrap().rotation.unwrap();
    assert_eq!(preview.before, mode());
    assert_eq!(preview.target, mode().oem_rotation().unwrap());
    controller.apply_rotation(&mode(), 100).unwrap();
    let target = mode().oem_rotation().unwrap();
    assert_eq!(io.list().unwrap()[0].current, target);
    assert!(controller.read().unwrap().pending.unwrap().rotation);
    controller.tick(15_100).unwrap();
    assert_eq!(io.list().unwrap()[0].current, mode());
    controller.apply_rotation(&mode(), 16_000).unwrap();
    controller.keep(2, &target, 16_001).unwrap();
    assert!(controller.read().unwrap().pending.is_none());
    controller.restore().unwrap();
    assert_eq!(io.list().unwrap()[0].current, mode());
}

#[test]
fn cancel_restores_last_kept_mode_and_rejects_old_trial_ids() {
    let (controller, io) = setup();
    controller.apply_rotation(&mode(), 0).unwrap();
    let rotated = mode().oem_rotation().unwrap();
    controller.keep(1, &rotated, 1).unwrap();
    controller.apply_rotation(&rotated, 2).unwrap();
    assert!(controller.cancel(1).is_err());
    assert!(controller.keep(1, &mode(), 3).is_err());
    assert_eq!(io.list().unwrap()[0].current, mode());
    controller.cancel(2).unwrap();
    assert_eq!(io.list().unwrap()[0].current, rotated);
    controller.restore().unwrap();
    assert_eq!(io.list().unwrap()[0].current, mode());
}

#[test]
fn rotation_rejects_stale_nonprimary_and_ambiguous_primary_without_writing() {
    let (controller, io) = setup();
    let mut stale = mode();
    stale.orientation = 2;
    assert!(controller.apply_rotation(&stale, 0).is_err());
    io.0.lock().unwrap().modes[0].primary = false;
    assert!(controller.apply_rotation(&mode(), 0).is_err());
    assert!(controller.primary().is_err());
    let mut second = io.0.lock().unwrap().modes[0].clone();
    second.current.device = "DISPLAY2".into();
    second.primary = true;
    let mut data = io.0.lock().unwrap();
    data.modes[0].primary = true;
    data.modes.push(second);
    drop(data);
    assert!(controller.apply_rotation(&mode(), 0).is_err());
    assert!(controller.primary().is_err());
    assert_eq!(io.0.lock().unwrap().writes, 0);
}

#[test]
fn rotation_driver_test_failure_and_external_changes_are_not_overwritten() {
    let (controller, io) = setup();
    io.0.lock().unwrap().test_fail = true;
    assert!(controller.apply_rotation(&mode(), 0).is_err());
    assert_eq!(io.0.lock().unwrap().writes, 0);
    io.0.lock().unwrap().test_fail = false;
    controller.apply_rotation(&mode(), 0).unwrap();
    io.0.lock().unwrap().modes[0].current.frequency = 144;
    assert!(controller.tick(15_000).is_err());
    assert_eq!(io.0.lock().unwrap().modes[0].current.frequency, 144);
    assert!(controller.read().unwrap().pending.is_some());
}

#[test]
fn automatic_trial_reports_each_outcome_once_and_blocks_other_owners() {
    let (controller, io) = setup();
    assert_eq!(controller.begin_automatic(&mode(), 60, 0).unwrap(), None);
    let first = controller
        .begin_automatic(&mode(), 144, 0)
        .unwrap()
        .unwrap();
    let pending = controller.read().unwrap().pending.unwrap();
    assert!(pending.automatic);
    assert_eq!(pending.before, vec![mode()]);
    assert!(controller.apply_rotation(&mode(), 1).is_err());
    let target = io.list().unwrap()[0].current.clone();
    assert!(controller.keep(first, &target, 1).is_err());
    controller.keep_automatic(first, &target, 1).unwrap();
    assert!(controller.apply(&target, 60, 2).is_err());
    assert!(controller.automatic_outcome(first + 1).is_err());
    let outcome = controller.automatic_outcome(first).unwrap().unwrap();
    assert_eq!(outcome.result, AutomaticResult::Kept);
    assert_eq!(controller.automatic_outcome(first).unwrap(), Some(outcome));
    controller.clear_automatic_outcome(outcome).unwrap();
    assert!(controller.automatic_outcome(first).unwrap().is_none());
    let second = controller.begin_automatic(&target, 60, 3).unwrap().unwrap();
    controller.tick(15_003).unwrap();
    assert_eq!(io.list().unwrap()[0].current, target);
    assert_eq!(
        controller
            .automatic_outcome(second)
            .unwrap()
            .unwrap()
            .result,
        AutomaticResult::Restored
    );
}

#[test]
fn automatic_cancel_requires_current_id_and_does_not_replace_external_changes() {
    let (controller, io) = setup();
    let id = controller
        .begin_automatic(&mode(), 144, 0)
        .unwrap()
        .unwrap();
    assert!(controller.cancel(id + 1).is_err());
    io.0.lock().unwrap().modes[0].current.identity = "other-panel".into();
    assert!(controller.cancel(id).is_err());
    assert!(controller.automatic_outcome(id).unwrap().is_none());
    io.0.lock().unwrap().modes[0].current.identity = "panel-a".into();
    controller.cancel(id).unwrap();
    assert_eq!(
        controller.automatic_outcome(id).unwrap().unwrap().result,
        AutomaticResult::Restored
    );
}
