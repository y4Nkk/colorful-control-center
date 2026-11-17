use super::{
    model::*,
    restore,
    store::FileStore,
    transaction::{self, Io},
};
use crate::hal::power_plan::{BatteryLevel, SCHEMES};
use anyhow::{Result, bail};
use std::{cell::RefCell, collections::BTreeMap};

const CUSTOM: u128 = 12345;
struct Memory(RefCell<State>);
struct State {
    active: u128,
    plans: BTreeMap<u128, [[u32; 2]; 3]>,
    cache: [u8; 3],
    writes: Vec<String>,
    fail_at: Vec<usize>,
    activations: Vec<u128>,
    writable: bool,
    active_reads: usize,
    switch_at: Option<(usize, u128)>,
    fail_activation: bool,
}
fn index(level: BatteryLevel) -> usize {
    match level {
        BatteryLevel::Low => 0,
        BatteryLevel::Critical => 1,
        BatteryLevel::Reserve => 2,
    }
}
impl Io for Memory {
    fn active(&self) -> Result<u128> {
        let mut state = self.0.borrow_mut();
        state.active_reads += 1;
        if let Some((at, target)) = state.switch_at {
            if state.active_reads == at {
                state.active = target;
            }
        }
        Ok(state.active)
    }
    fn schemes(&self) -> Result<Vec<u128>> {
        Ok(self.0.borrow().plans.keys().copied().collect())
    }
    fn levels(&self, plan: u128, level: BatteryLevel) -> Result<[u32; 2]> {
        Ok(self
            .0
            .borrow()
            .plans
            .get(&plan)
            .ok_or_else(|| anyhow::anyhow!("隔离计划不存在"))?[index(level)])
    }
    fn write(&self, plan: u128, level: BatteryLevel, ac: bool, value: u32) -> Result<()> {
        let mut state = self.0.borrow_mut();
        state
            .plans
            .get_mut(&plan)
            .ok_or_else(|| anyhow::anyhow!("隔离计划不存在"))?[index(level)][usize::from(!ac)] =
            value;
        state
            .writes
            .push(format!("plan:{plan}:{level:?}:{ac}:{value}"));
        if state.fail_at.contains(&state.writes.len()) {
            bail!("隔离写入后失败");
        }
        Ok(())
    }
    fn cache(&self) -> Result<[u8; 3]> {
        Ok(self.0.borrow().cache)
    }
    fn write_cache(&self, index: usize, value: u8) -> Result<()> {
        let mut state = self.0.borrow_mut();
        state.cache[index] = value;
        state.writes.push(format!("cache:{index}:{value}"));
        if state.fail_at.contains(&state.writes.len()) {
            bail!("隔离写入后失败");
        }
        Ok(())
    }
    fn activate(&self, plan: u128) -> Result<()> {
        let mut state = self.0.borrow_mut();
        if state.active != plan {
            bail!("隔离活动计划变化");
        }
        state.activations.push(plan);
        if state.fail_activation {
            state.fail_activation = false;
            bail!("刷新计划失败");
        }
        Ok(())
    }
    fn ensure_writable(&self) -> Result<()> {
        if !self.0.borrow().writable {
            bail!("校准中不可写");
        }
        Ok(())
    }
}
fn fixture() -> (Memory, tempfile::TempDir, FileStore) {
    let temp = tempfile::tempdir().unwrap();
    let store = FileStore(temp.path().join("battery-thresholds.json"));
    let plans = BTreeMap::from([
        (CUSTOM, [[10, 11], [5, 7], [4, 6]]),
        (SCHEMES[0], [[9, 0], [2, 3], [1, 4]]),
        (SCHEMES[2], [[30, 31], [8, 9], [6, 8]]),
    ]);
    (
        Memory(RefCell::new(State {
            active: CUSTOM,
            plans,
            cache: [0; 3],
            writes: vec![],
            fail_at: vec![],
            activations: vec![],
            writable: true,
            active_reads: 0,
            switch_at: None,
            fail_activation: false,
        })),
        temp,
        store,
    )
}
fn apply(io: &Memory, store: &FileStore) -> Result<String> {
    let identity = transaction::preview(io, store).identity.unwrap();
    transaction::apply(io, store, &identity, &mut || Ok(()))
}
fn recover(io: &Memory, store: &FileStore) -> Result<()> {
    let mut record = store.load()?.unwrap();
    restore::restore(io, store, &mut record, &mut || Ok(()))
}

#[test]
fn writes_exact_selected_plans_and_restores_each_ac_dc_value_including_zero() {
    let (io, _temp, store) = fixture();
    let before = transaction::snapshot(&io).unwrap();
    let identity = before.identity().unwrap();
    apply(&io, &store).unwrap();
    let after = transaction::snapshot(&io).unwrap();
    assert_eq!(after.cache, [11, 7, 6]);
    assert_eq!(after.plans.len(), 3);
    assert!(
        after
            .plans
            .iter()
            .all(|plan| plan.values == [[25, 25], [20, 20], [22, 22]])
    );
    assert_eq!(
        transaction::preview(&io, &store).identity.as_deref(),
        Some(identity.as_str())
    );
    let writes = io.0.borrow().writes.len();
    assert!(
        transaction::apply(&io, &store, &identity, &mut || Ok(()))
            .unwrap()
            .contains("未重复写入")
    );
    assert_eq!(io.0.borrow().writes.len(), writes);
    recover(&io, &store).unwrap();
    assert_eq!(transaction::snapshot(&io).unwrap(), before);
    assert!(store.load().unwrap().is_none());
    assert!(io.0.borrow().activations.iter().all(|plan| *plan == CUSTOM));
}

#[test]
fn existing_oem_cache_and_stale_confirmation_refuse_without_writes() {
    let (io, _temp, store) = fixture();
    let identity = transaction::preview(&io, &store).identity.unwrap();
    io.0.borrow_mut().cache[1] = 7;
    assert!(transaction::preview(&io, &store).identity.is_none());
    assert!(transaction::apply(&io, &store, &identity, &mut || Ok(())).is_err());
    assert!(io.0.borrow().writes.is_empty());
    assert!(store.load().unwrap().is_none());
}

#[test]
fn partial_failure_after_write_is_compensated_and_first_baseline_not_lost() {
    let (io, _temp, store) = fixture();
    let before = transaction::snapshot(&io).unwrap();
    io.0.borrow_mut().fail_at = vec![8];
    let error = apply(&io, &store).unwrap_err();
    assert!(format!("{error:#}").contains("已恢复"));
    assert_eq!(transaction::snapshot(&io).unwrap(), before);
    assert!(store.load().unwrap().is_none());
}

#[test]
fn failed_compensation_keeps_journal_and_retry_restores_originals() {
    let (io, _temp, store) = fixture();
    let before = transaction::snapshot(&io).unwrap();
    io.0.borrow_mut().fail_at = vec![8, 9];
    assert!(apply(&io, &store).is_err());
    assert_eq!(store.load().unwrap().unwrap().phase, Phase::Restoring);
    assert!(transaction::preview(&io, &store).identity.is_none());
    recover(&io, &store).unwrap();
    assert_eq!(transaction::snapshot(&io).unwrap(), before);
}

#[test]
fn interruption_between_journal_and_write_can_be_recovered_from_disk() {
    let (io, _temp, store) = fixture();
    let before = transaction::snapshot(&io).unwrap();
    let mut record = Record::new(before.clone()).unwrap();
    record.owned[0][0][1] = true;
    record.cache_owned[0] = true;
    store.save(None, &record).unwrap();
    io.0.borrow_mut().plans.get_mut(&CUSTOM).unwrap()[0][1] = 25;
    io.0.borrow_mut().cache[0] = 11;
    assert!(transaction::preview(&io, &store).restore_revision.is_some());
    recover(&io, &store).unwrap();
    assert_eq!(transaction::snapshot(&io).unwrap(), before);
}

#[test]
fn external_owned_change_is_never_overwritten_and_journal_remains() {
    let (io, _temp, store) = fixture();
    apply(&io, &store).unwrap();
    io.0.borrow_mut().plans.get_mut(&CUSTOM).unwrap()[0][0] = 77;
    let count = io.0.borrow().writes.len();
    let view = transaction::preview(&io, &store);
    assert!(view.identity.is_none() && view.restore_revision.is_none());
    assert!(recover(&io, &store).is_err());
    assert_eq!(io.0.borrow().writes.len(), count);
    assert!(store.load().unwrap().is_some());
}

#[test]
fn unowned_field_external_change_survives_recovery() {
    let (io, _temp, store) = fixture();
    io.0.borrow_mut().plans.get_mut(&CUSTOM).unwrap()[0][0] = 25;
    apply(&io, &store).unwrap();
    io.0.borrow_mut().plans.get_mut(&CUSTOM).unwrap()[0][0] = 88;
    assert!(transaction::preview(&io, &store).restore_revision.is_some());
    recover(&io, &store).unwrap();
    assert_eq!(io.0.borrow().plans[&CUSTOM][0][0], 88);
}

#[test]
fn cancellation_before_first_output_cleans_journal_without_writes() {
    let (io, _temp, store) = fixture();
    let identity = transaction::preview(&io, &store).identity.unwrap();
    let mut calls = 0;
    assert!(
        transaction::apply(&io, &store, &identity, &mut || {
            calls += 1;
            if calls > 1 {
                bail!("许可撤销");
            }
            Ok(())
        })
        .is_err()
    );
    assert!(io.0.borrow().writes.is_empty());
    assert!(store.load().unwrap().is_none());
}

#[test]
fn corrupt_or_replaced_journal_is_preserved() {
    let (io, _temp, store) = fixture();
    std::fs::write(&store.0, b"{invalid").unwrap();
    let view = transaction::preview(&io, &store);
    assert!(view.identity.is_none() && view.restore_error.is_some());
    assert_eq!(std::fs::read(&store.0).unwrap(), b"{invalid");
    assert!(io.0.borrow().writes.is_empty());
}

#[test]
fn battery_threshold_events_require_their_own_authorization() {
    use crate::hotkeys::model::{Capability, Facts, Record, Scope, Settings};
    use ccc_core::oem_events::Action;
    for code in [209, 210] {
        assert_eq!(
            Scope::for_action(Action::BatteryThreshold(code)),
            Scope::BatteryThresholds
        );
    }
    let mut facts = Facts {
        mode_cycle: None,
        panel_hotkey: None,
        screen_brightness: None,
        battery_low_brightness: None,
        battery_low_action: None,
        lid_action: None,
        battery_alert: None,
        gpu_turbo: None,
        battery_thresholds: None,
        pd_event: None,
        capabilities: vec![Capability {
            scope: Scope::BatteryThresholds,
            identity: Some("confirmed-plan".into()),
            reason: None,
        }],
        blocked_reason: None,
    };
    let settings = Settings {
        enabled: true,
        actions: vec![Scope::BatteryThresholds],
        osd: true,
    };
    let record = Record::authorize(settings, &facts).unwrap();
    record.verify(&facts).unwrap();
    facts.capabilities[0].identity = Some("changed-plan-baseline".into());
    assert!(record.verify(&facts).is_err());
}

#[test]
fn changing_active_plan_during_preview_cannot_produce_confirmation() {
    let (io, _temp, store) = fixture();
    io.0.borrow_mut().switch_at = Some((2, SCHEMES[0]));
    assert!(transaction::preview(&io, &store).identity.is_none());
    assert!(io.0.borrow().writes.is_empty());
}

#[test]
fn changing_active_plan_mid_apply_restores_fields_without_switching_it_back() {
    let (io, _temp, store) = fixture();
    let before = io.0.borrow().plans.clone();
    let identity = transaction::preview(&io, &store).identity.unwrap();
    let mut calls = 0;
    let result = transaction::apply(&io, &store, &identity, &mut || {
        calls += 1;
        if calls == 6 {
            io.0.borrow_mut().active = SCHEMES[0];
        }
        Ok(())
    });
    assert!(result.is_err());
    assert_eq!(io.0.borrow().active, SCHEMES[0]);
    assert_eq!(io.0.borrow().plans, before);
    assert!(
        io.0.borrow()
            .activations
            .iter()
            .all(|plan| *plan == SCHEMES[0])
    );
    assert_eq!(io.0.borrow().cache, [0; 3]);
    assert!(store.load().unwrap().is_none());
}

#[test]
fn failed_active_plan_refresh_restores_fields_and_allows_retry() {
    let (io, _temp, store) = fixture();
    let before = transaction::snapshot(&io).unwrap();
    io.0.borrow_mut().fail_activation = true;
    assert!(apply(&io, &store).is_err());
    assert_eq!(transaction::snapshot(&io).unwrap(), before);
    assert!(store.load().unwrap().is_none());
    apply(&io, &store).unwrap();
}
