use super::*;
use crate::gpu_power::mode as gpu;
use std::cell::RefCell;

pub(in crate::powerctl::mode::supply) struct Memory(
    pub(in crate::powerctl::mode::supply) RefCell<Data>,
);
pub(in crate::powerctl::mode::supply) struct Data {
    pub(in crate::powerctl::mode::supply) review: Review,
    pub(in crate::powerctl::mode::supply) snapshot: StoredModeSnapshot,
    pub(in crate::powerctl::mode::supply) writes: usize,
    pub(super) fail_after: bool,
    pub(super) mutate_before_restore: bool,
    pub(super) plans: Vec<PlanValue>,
    pub(super) clocks: Option<crate::gpuctl::turbo::mode::RawState>,
    pub(super) clock_writes: usize,
    pub(super) clock_reads: usize,
    pub(super) driver_identity: String,
    pub(super) driver_error: Option<String>,
}
pub(super) fn target(mode: u8) -> Target {
    Target {
        from_mode: None,
        gpu: None,
        revision: None,
        unavailable_reason: None,
        ac_mode: None,
        energy: Some(energy(mode)),
        mode,
        turbo: false,
        dtt: false,
        fan_mode: 0,
        fan_saved_mode: 0,
        fan_offset_pct: Some(0),
        aspm: None,
        power_plan: None,
    }
}
pub(super) fn snapshot(
    mode: u8,
    offset: u8,
    energy: crate::powerctl::mode::EnergyState,
) -> StoredModeSnapshot {
    serde_json::from_value(serde_json::json!({"core":{"mode":mode,"command":if mode == 4 { 0 } else { mode },"preferences":[0,0],"acMode":null},
        "fan":{"software":null,"package":null,"rpmPairs":null,"fields":[{"offset":5,"data":[0]}, {"offset":7,"data":[offset]}],"mode":0,
            "offsetRaw":ccc_core::fan::oem_duty_byte(f64::from(offset))},
        "system":{"active":format!("{:032x}", crate::hal::power_plan::SCHEMES[0]),"aspm":{"381b4222f69441f09685ff5bb260df2e":[0,1]}},
        "energy":energy})).unwrap()
}
fn energy(mode: u8) -> crate::powerctl::mode::energy::State {
    crate::powerctl::mode::energy::State {
        hardware_p_state: false,
        energy_star: Some(match mode {
            0 | 3 => 2,
            1 | 4 => 1,
            2 => 0,
            _ => unreachable!(),
        }),
    }
}
impl Memory {
    pub(super) fn set_energy(&self, state: crate::powerctl::mode::energy::State) {
        let mut data = self.0.borrow_mut();
        let mut mode = data.snapshot.restore().unwrap();
        mode.energy = Some(state);
        data.snapshot = mode.stored();
        data.review.current_energy = state;
    }
    fn apply_gpu_action(&self, attempt: &Attempt, stage: gpu::Stage) {
        if let Some(action) = attempt.gpu_action.filter(|action| action.stage == stage) {
            let mut data = self.0.borrow_mut();
            let device = data.review.gpu.current.as_mut().unwrap();
            device.enabled = action.target.enabled;
            device.automatic_power_off = action.target.automatic_power_off;
            device.problem = if device.enabled { 0 } else { 22 };
        }
    }
}
impl Io for Memory {
    fn review(&self, scope: Scope) -> Result<Option<Review>> {
        let data = self.0.borrow();
        let mut review = data.review.clone();
        if scope.skips_gpu() {
            review.clocks = None;
        } else if let Some(clocks) = &mut review.clocks {
            if let Some(driver) = &mut clocks.driver {
                if let Some(raw) = &data.clocks {
                    driver.current = raw.clone();
                }
                driver.identity = data.driver_identity.clone();
            }
            clocks.driver_error = data.driver_error.clone();
        }
        Ok(Some(review))
    }
    fn device_identity(&self) -> Result<String> {
        Ok(self.0.borrow().review.device_identity.clone())
    }
    fn snapshot(&self, _: &Target) -> Result<StoredModeSnapshot> {
        Ok(self.0.borrow().snapshot.clone())
    }
    fn initial_plans(&self, _: &Review, _: &StoredModeSnapshot) -> Result<Vec<PlanValue>> {
        Ok(self.0.borrow().plans.clone())
    }
    fn plans(&self, initial: &[PlanValue]) -> Result<Vec<PlanValue>> {
        let values = self.0.borrow().plans.clone();
        plans::changes(initial, &values)?;
        Ok(values)
    }
    fn gpu_power(&self) -> Result<Option<gpu::State>> {
        self.0
            .borrow()
            .review
            .gpu
            .current
            .as_ref()
            .map(gpu::State::from_device)
            .transpose()
    }
    fn clocks(&self) -> Result<Option<crate::gpuctl::turbo::mode::RawState>> {
        let mut data = self.0.borrow_mut();
        if data
            .review
            .gpu
            .current
            .as_ref()
            .is_some_and(|gpu| !gpu.enabled)
        {
            return Ok(None);
        }
        data.clock_reads += 1;
        if let Some(error) = &data.driver_error {
            bail!("{error}");
        }
        Ok(data.clocks.clone())
    }
    fn review_gpu_restore(
        &self,
        initial: Option<&gpu::State>,
        expected: Option<&gpu::State>,
    ) -> Result<()> {
        match (
            initial,
            expected,
            self.0.borrow().review.gpu.current.as_ref(),
        ) {
            (None, None, None) => Ok(()),
            (Some(initial), Some(expected), Some(current)) if initial.id == expected.id => {
                crate::gpu_power::operation::preflight(current, expected.target, initial.target)
            }
            _ => bail!("隔离独显恢复设备已变化"),
        }
    }
    fn apply(
        &self,
        attempt: &Attempt,
        _: &Target,
        checkpoint: &mut dyn FnMut(
            &crate::gpuctl::turbo::mode::Write,
            ClockOrigin,
            &str,
        ) -> Result<()>,
        permit: &mut dyn FnMut() -> Result<()>,
    ) -> Result<()> {
        permit()?;
        let before = &attempt.before;
        let target = &attempt.target;
        if self.gpu_power()? != before.gpu_power {
            bail!("独显前态已变化");
        }
        let mut data = self.0.borrow_mut();
        if data.snapshot != before.mode
            || data.plans != before.plans
            || data.review.current_energy != before.mode.energy()?
        {
            bail!("前态已变化");
        }
        data.writes += 1;
        let index = (data.writes - 1) % data.plans.len();
        data.plans[index].aspm = [u32::from(target.mode), u32::from(target.mode)];
        data.snapshot = snapshot(
            target.mode,
            target.fan_offset_pct.unwrap(),
            before.mode.energy()?,
        );
        data.review.current_mode = target.mode;
        drop(data);
        self.apply_gpu_action(attempt, gpu::Stage::BeforeMode);
        self.apply_clocks(attempt, checkpoint)?;
        self.set_energy(
            target
                .energy
                .ok_or_else(|| anyhow::anyhow!("缺少节能目标"))?,
        );
        self.apply_gpu_action(attempt, gpu::Stage::AfterMode);
        if self.0.borrow().fail_after {
            bail!("隔离部分失败");
        }
        Ok(())
    }
    fn restore(
        &self,
        initial: &Initial,
        _: &Target,
        expected: &Snapshot,
        permit: &mut dyn FnMut() -> Result<()>,
    ) -> Result<()> {
        permit()?;
        if self.gpu_power()? != expected.gpu_power {
            bail!("独显恢复前态已变化");
        }
        if ClockRead::for_observation(&expected.gpu_clocks)?.capture(self)? != expected.gpu_clocks {
            bail!("GPU频率恢复前态已变化");
        }
        let mut data = self.0.borrow_mut();
        if data.mutate_before_restore {
            data.snapshot = snapshot(3, 15, data.snapshot.energy()?);
        }
        if data.snapshot != expected.mode
            || data.plans != expected.plans
            || data.review.current_energy != expected.mode.energy()?
        {
            bail!("恢复预览后已变化");
        }
        data.writes += 1;
        data.snapshot = initial.mode.clone();
        data.plans = initial.plans.clone();
        data.review.current_energy = initial.mode.energy()?;
        if let Some(clocks) = &initial.gpu_clocks {
            data.clocks = Some(clocks.state.clone());
            data.clock_writes += 1;
        }
        match (&initial.gpu_power, &mut data.review.gpu.current) {
            (Some(initial), Some(current)) => {
                current.enabled = initial.target.enabled;
                current.automatic_power_off = initial.target.automatic_power_off;
                current.problem = if current.enabled { 0 } else { 22 };
            }
            (None, None) => (),
            _ => bail!("独显恢复设备已变化"),
        }
        data.review.current_mode = serde_json::to_value(&initial.mode)?["core"]["mode"]
            .as_u64()
            .unwrap() as u8;
        Ok(())
    }
}
pub(in crate::powerctl::mode::supply) fn setup() -> (tempfile::TempDir, FileStore, Memory) {
    let dir = tempfile::tempdir().unwrap();
    let store = FileStore(dir.path().join("supply-mode.json"));
    let ac = target(2);
    let battery = target(1);
    let review = Review {
        device_identity: "a".repeat(64),
        identity: "b".repeat(64),
        revision: "r".into(),
        current_mode: 2,
        ac_mode: 2,
        on_ac: Some(false),
        ac: Some(ac.clone()),
        battery: Some(battery.clone()),
        startup_ac: Some(ac.clone()),
        startup_battery: Some(battery.clone()),
        resume_ac: Some(ac.clone()),
        resume_battery: Some(battery.clone()),
        targets: vec![battery, ac],
        software_fan_active: false,
        current_energy: crate::powerctl::mode::energy::State {
            hardware_p_state: false,
            energy_star: Some(0),
        },
        dc_mode_power: true,
        gpu: crate::powerctl::mode::supply::gpu::review(false, &[], true, [None; 6]).unwrap(),
        clocks: Some(crate::gpuctl::turbo::mode::review(
            crate::gpuctl::turbo::mode::test_policy(),
            [None; 6],
        )),
    };
    (
        dir,
        store,
        Memory(RefCell::new(Data {
            review,
            snapshot: snapshot(2, 0, energy(2)),
            writes: 0,
            fail_after: false,
            mutate_before_restore: false,
            plans: vec![
                PlanValue {
                    scheme: format!("{:032x}", crate::hal::power_plan::SCHEMES[0]),
                    aspm: [0, 1],
                },
                PlanValue {
                    scheme: format!("{:032x}", crate::hal::power_plan::SCHEMES[2]),
                    aspm: [1, 2],
                },
            ],
            clocks: None,
            clock_writes: 0,
            clock_reads: 0,
            driver_identity: "d".repeat(64),
            driver_error: None,
        })),
    )
}
pub(super) fn enable(io: &Memory, store: &FileStore) {
    let view = transaction::read(io, store).unwrap();
    assert!(view.can_enable);
    transaction::enable(io, store, &view.revision, &mut || Ok(())).unwrap();
}
