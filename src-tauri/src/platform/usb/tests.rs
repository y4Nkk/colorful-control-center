use super::*;
use model::{PlanId, Request, Targets};

struct Memory {
    state: Mutex<State>,
    reads: std::sync::atomic::AtomicUsize,
}
struct State {
    firmware: Option<u8>,
    active: u128,
    plans: BTreeMap<u128, [u32; 2]>,
    writes: usize,
    failures: Vec<usize>,
    blocked: Option<String>,
    switch_at: Option<usize>,
    external_field: bool,
    activations: Vec<u128>,
}
impl Memory {
    fn new() -> Self {
        Self {
            state: Mutex::new(State {
                firmware: Some(1),
                active: 999,
                plans: SCHEMES
                    .into_iter()
                    .chain([999])
                    .map(|id| (id, [0, 0]))
                    .collect(),
                writes: 0,
                failures: vec![],
                blocked: None,
                switch_at: None,
                external_field: false,
                activations: vec![],
            }),
            reads: Default::default(),
        }
    }
    fn frame(&self, baseline: &Baseline) -> Frame {
        capture(self, !baseline.is_empty()).unwrap().unwrap()
    }
    fn request(&self, baseline: &Baseline, ac: Option<bool>, dc: Option<bool>) -> Request {
        Request {
            expected_revision: self.frame(baseline).revision().unwrap(),
            values: Targets { ac, dc },
        }
    }
    fn apply(&self, baseline: &mut Baseline, ac: Option<bool>, dc: Option<bool>) -> Result<()> {
        let request = self.request(baseline, ac, dc);
        transaction::apply(self, baseline, self.frame(baseline), &request)
    }
    fn finish(state: &mut State) -> Result<()> {
        state.writes += 1;
        if state.switch_at == Some(state.writes) {
            state.active = SCHEMES[2];
        }
        if state.external_field && state.writes == 1 {
            state.plans.get_mut(&SCHEMES[0]).unwrap()[1] = 1;
        }
        if state.failures.contains(&state.writes) {
            bail!("隔离电源API部分成功后失败");
        }
        Ok(())
    }
}
impl PowerAccess for Memory {
    fn lid_action(&self, _: u128) -> Result<[u32; 2]> {
        anyhow::bail!("此隔离提供者未注入合盖计划动作")
    }
    fn battery_action(&self, _: u128, _: crate::hal::power_plan::BatteryAction) -> Result<u32> {
        bail!("此隔离提供者未注入电池计划动作")
    }
    fn battery_level(&self, _: u128, _: crate::hal::power_plan::BatteryLevel) -> Result<[u32; 2]> {
        bail!("此隔离提供者未注入电池计划阈值")
    }
    fn set_battery_level(
        &self,
        _: u128,
        _: crate::hal::power_plan::BatteryLevel,
        _: bool,
        _: u32,
    ) -> Result<()> {
        bail!("此隔离提供者未注入电池计划阈值")
    }
    fn brightness(&self, _: u128) -> anyhow::Result<[u32; 2]> {
        anyhow::bail!("此隔离提供者未注入屏幕亮度字段")
    }
    fn set_brightness(&self, _: u128, _: bool, _: u32) -> anyhow::Result<()> {
        anyhow::bail!("此隔离提供者未注入屏幕亮度字段")
    }

    fn active(&self) -> Result<u128> {
        self.reads
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Ok(self.state.lock().unwrap().active)
    }
    fn schemes(&self) -> Result<Vec<u128>> {
        self.reads
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Ok(self.state.lock().unwrap().plans.keys().copied().collect())
    }
    fn usb(&self, scheme: u128) -> Result<[u32; 2]> {
        self.reads
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.state
            .lock()
            .unwrap()
            .plans
            .get(&scheme)
            .copied()
            .context("隔离计划不存在")
    }
    fn set_usb(&self, scheme: u128, ac: bool, value: u32) -> Result<()> {
        let mut state = self.state.lock().unwrap();
        state.plans.get_mut(&scheme).context("隔离计划不存在")?[usize::from(!ac)] = value;
        Self::finish(&mut state)
    }
    fn aspm(&self, _: u128) -> Result<[u32; 2]> {
        bail!("USB事务不应读取ASPM")
    }
    fn set_aspm(&self, _: u128, _: bool, _: u32) -> Result<()> {
        bail!("USB事务不应写入ASPM")
    }
    fn activate(&self, scheme: u128) -> Result<()> {
        let mut state = self.state.lock().unwrap();
        state.active = scheme;
        state.activations.push(scheme);
        Self::finish(&mut state)
    }
}
impl Io for Memory {
    fn firmware(&self) -> Result<Option<u8>> {
        Ok(self.state.lock().unwrap().firmware)
    }
    fn power(&self) -> &dyn PowerAccess {
        self
    }
    fn ensure_write(&self) -> Result<()> {
        if let Some(reason) = &self.state.lock().unwrap().blocked {
            bail!(reason.clone());
        }
        Ok(())
    }
}
#[test]
fn original_three_plans_and_six_fields_change_without_selecting_a_different_active_plan() {
    let io = Memory::new();
    let mut baseline = Baseline::new();
    io.apply(&mut baseline, Some(true), Some(true)).unwrap();
    let state = io.state.lock().unwrap();
    assert_eq!(state.writes, 7);
    assert_eq!(state.active, 999);
    assert_eq!(state.plans[&999], [0, 0]);
    for id in SCHEMES {
        assert_eq!(state.plans[&id], [1, 1]);
    }
    assert_eq!(baseline.len(), 3);
    assert!(baseline.values().all(|v| *v == [Some(0), Some(0)]));
}
#[test]
fn targeted_ac_only_keeps_dc_and_restore_does_not_overwrite_later_unowned_dc_values() {
    let io = Memory::new();
    let mut baseline = Baseline::new();
    io.apply(&mut baseline, Some(true), None).unwrap();
    assert!(baseline.values().all(|v| *v == [Some(0), None]));
    {
        let mut state = io.state.lock().unwrap();
        for id in SCHEMES {
            state.plans.get_mut(&id).unwrap()[1] = 1;
        }
        state.active = SCHEMES[1];
    }
    let before = io.frame(&baseline);
    let revision = before.revision().unwrap();
    transaction::restore(&io, &baseline, before, &revision).unwrap();
    let state = io.state.lock().unwrap();
    assert_eq!(state.active, SCHEMES[1]);
    for id in SCHEMES {
        assert_eq!(state.plans[&id], [0, 1]);
    }
}
#[test]
fn each_partial_write_and_reload_failure_compensates_and_retains_attempted_initial_fields() {
    for failure in 1..=7 {
        let io = Memory::new();
        io.state.lock().unwrap().failures = vec![failure];
        let mut baseline = Baseline::new();
        assert!(io.apply(&mut baseline, Some(true), Some(true)).is_err());
        for id in SCHEMES {
            assert_eq!(io.usb(id).unwrap(), [0, 0]);
        }
        assert!(!baseline.is_empty());
        assert_eq!(io.active().unwrap(), 999);
    }
}
#[test]
fn outside_plan_switch_is_not_undone_by_compensation() {
    let io = Memory::new();
    io.state.lock().unwrap().switch_at = Some(2);
    let mut baseline = Baseline::new();
    assert!(
        io.apply(&mut baseline, Some(true), Some(true))
            .unwrap_err()
            .to_string()
            .contains("活动电源计划")
    );
    assert_eq!(io.active().unwrap(), SCHEMES[2]);
    assert_eq!(io.state.lock().unwrap().activations, vec![SCHEMES[2]]);
    assert_eq!(baseline.len(), 1);
    for id in SCHEMES {
        assert_eq!(io.usb(id).unwrap(), [0, 0]);
    }
}
#[test]
fn outside_change_to_not_yet_attempted_field_is_preserved() {
    let io = Memory::new();
    io.state.lock().unwrap().external_field = true;
    let mut baseline = Baseline::new();
    assert!(io.apply(&mut baseline, Some(true), Some(true)).is_err());
    assert_eq!(io.usb(SCHEMES[0]).unwrap(), [0, 1]);
    assert_eq!(baseline[&PlanId::Balanced], [Some(0), None]);
}
#[test]
fn failed_compensation_keeps_retryable_initial_values_and_restore_attempts_every_field() {
    let io = Memory::new();
    io.state.lock().unwrap().failures = vec![2, 3];
    let mut baseline = Baseline::new();
    assert!(io.apply(&mut baseline, Some(true), Some(true)).is_err());
    assert_eq!(baseline[&PlanId::Balanced], [Some(0), Some(0)]);
    let before = io.frame(&baseline);
    let revision = before.revision().unwrap();
    transaction::restore(&io, &baseline, before, &revision).unwrap();
    assert_eq!(io.usb(SCHEMES[0]).unwrap(), [0, 0]);
}
#[test]
fn missing_plans_are_reported_and_never_created_but_missing_initial_plan_blocks_restore() {
    let io = Memory::new();
    io.state.lock().unwrap().plans.remove(&SCHEMES[1]);
    let mut baseline = Baseline::new();
    let state = profile(&io, &baseline).unwrap().unwrap();
    assert_eq!(state.missing, vec![PlanId::HighPerformance]);
    io.apply(&mut baseline, Some(true), Some(true)).unwrap();
    assert_eq!(baseline.len(), 2);
    io.state.lock().unwrap().plans.remove(&SCHEMES[0]);
    let before = io.frame(&baseline);
    let revision = before.revision().unwrap();
    let count = io.state.lock().unwrap().writes;
    assert!(transaction::restore(&io, &baseline, before, &revision).is_err());
    assert_eq!(io.state.lock().unwrap().writes, count);
    assert!(
        profile(&io, &baseline)
            .unwrap()
            .unwrap()
            .restore_blocked_reason
            .is_some()
    );
}
#[test]
fn unsupported_capability_performs_no_system_reads_but_existing_recovery_survives_capability_loss()
{
    let io = Memory::new();
    io.state.lock().unwrap().firmware = None;
    assert!(profile(&io, &Baseline::new()).unwrap().is_none());
    assert_eq!(io.reads.load(std::sync::atomic::Ordering::Relaxed), 0);
    io.state.lock().unwrap().firmware = Some(1);
    let mut baseline = Baseline::new();
    io.apply(&mut baseline, Some(true), None).unwrap();
    io.state.lock().unwrap().firmware = None;
    let state = profile(&io, &baseline).unwrap().unwrap();
    assert!(!state.supported);
    assert!(state.blocked_reason.is_some());
    assert!(state.restore_blocked_reason.is_none());
    let before = io.frame(&baseline);
    let revision = before.revision().unwrap();
    transaction::restore(&io, &baseline, before, &revision).unwrap();
}
#[test]
fn stale_review_invalid_targets_unknown_fields_and_write_guards_do_not_mutate() {
    let io = Memory::new();
    let mut baseline = Baseline::new();
    let mut request = io.request(&baseline, Some(true), None);
    io.state.lock().unwrap().firmware = Some(2);
    let frame = io.frame(&baseline);
    assert!(transaction::apply(&io, &mut baseline, frame, &request).is_err());
    request = io.request(&baseline, None, None);
    let frame = io.frame(&baseline);
    assert!(transaction::apply(&io, &mut baseline, frame, &request).is_err());
    for reason in ["电池校准期间不能修改", "原厂控制者仍在运行"] {
        io.state.lock().unwrap().blocked = Some(reason.into());
        assert!(io.apply(&mut baseline, Some(true), Some(true)).is_err());
    }
    assert!(
        serde_json::from_str::<Request>(
            r#"{"expectedRevision":"x","values":{"ac":true,"dc":false,"legacy":1}}"#
        )
        .is_err()
    );
    assert!(baseline.is_empty());
    assert_eq!(io.state.lock().unwrap().writes, 0);
}
#[test]
fn raw_unknown_field_is_visible_and_only_known_selected_fields_can_change() {
    let io = Memory::new();
    io.state.lock().unwrap().plans.get_mut(&SCHEMES[0]).unwrap()[0] = 2;
    let mut baseline = Baseline::new();
    let state = profile(&io, &baseline).unwrap().unwrap();
    assert!(state.plans[0].ac.is_none());
    assert!(!state.warnings.is_empty());
    assert!(io.apply(&mut baseline, Some(false), None).is_err());
    io.apply(&mut baseline, None, Some(true)).unwrap();
    assert_eq!(io.usb(SCHEMES[0]).unwrap(), [2, 1]);
}
#[test]
fn capability_bytes_do_not_confuse_pd_or_lan_and_noop_does_not_capture_baseline() {
    let mut bytes = [0; 24];
    bytes[0] = 1;
    bytes[23] = 2;
    assert!(!model::supported(&bytes).unwrap());
    bytes[21] = 128;
    assert!(!model::supported(&bytes).unwrap());
    bytes[23] |= 1;
    assert!(model::supported(&bytes).unwrap());
    bytes[0] = 2;
    assert!(model::supported(&bytes).is_err());
    let io = Memory::new();
    let mut baseline = Baseline::new();
    io.apply(&mut baseline, Some(false), Some(false)).unwrap();
    assert!(baseline.is_empty());
    assert_eq!(io.state.lock().unwrap().writes, 0);
}
