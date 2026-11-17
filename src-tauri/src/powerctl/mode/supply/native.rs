use super::{
    Review, Scope, Target,
    plans::{self, PlanValue},
    record::{Attempt, ClockRead, Initial, Snapshot},
    transaction::{self, Io},
};
use crate::gpuctl::turbo::mode as gpu_mode;
use crate::{
    gpu_power::{self, mode as node_mode},
    gpuctl::GpuController,
    hal::Hal,
    powerctl::mode::{self, workflow::StoredModeSnapshot},
};
use anyhow::{Result, bail};
use gpu_mode::ClockOrigin;
mod gpu;

pub(super) struct Native<'a> {
    hal: &'a Hal,
    links: mode::links::Session<'a>,
}
impl<'a> Native<'a> {
    pub(super) fn new(
        hal: &'a Hal,
        clocks: &'a GpuController,
        power: &'a gpu_power::Controller,
    ) -> Result<Self> {
        Ok(Self {
            hal,
            links: mode::links::Context::native(hal, clocks, power).session()?,
        })
    }
}

impl Io for Native<'_> {
    fn review(&self, scope: Scope) -> Result<Option<Review>> {
        let Some(mut review) = super::read(self.hal, self.links.context.devices()?, scope)? else {
            return Ok(None);
        };
        let modes = scope.clock_modes(&review);
        if let Some(clocks) = &mut review.clocks {
            self.links.context.inspect(
                clocks,
                modes,
                review.on_ac,
                review
                    .gpu
                    .current
                    .as_ref()
                    .is_some_and(|device| !device.enabled),
            );
        }
        review.revision = super::digest(&(&review.revision, &review.identity, &review.clocks))?;
        Ok(Some(review))
    }
    fn device_identity(&self) -> Result<String> {
        super::device_identity(self.hal)
    }
    fn snapshot(&self, target: &Target) -> Result<StoredModeSnapshot> {
        Ok(mode::workflow::checkpoint_for_supply(
            self.hal,
            target.mode,
            target.power_plan.is_some(),
        )?
        .before
        .stored())
    }
    fn initial_plans(
        &self,
        review: &Review,
        initial: &StoredModeSnapshot,
    ) -> Result<Vec<PlanValue>> {
        plans::initial(self.hal.system_power.as_ref(), review, initial)
    }
    fn plans(&self, initial: &[PlanValue]) -> Result<Vec<PlanValue>> {
        plans::current(self.hal.system_power.as_ref(), initial)
    }
    fn gpu_power(&self) -> Result<Option<node_mode::State>> {
        self.links
            .context
            .device()?
            .as_ref()
            .map(node_mode::State::from_device)
            .transpose()
    }
    fn clocks(&self) -> Result<Option<gpu_mode::RawState>> {
        self.links.read_clocks()
    }
    fn review_gpu_restore(
        &self,
        initial: Option<&node_mode::State>,
        current: Option<&node_mode::State>,
    ) -> Result<()> {
        self.links.preflight_restore(initial, current)
    }
    fn apply(
        &self,
        attempt: &Attempt,
        sampling: &Target,
        checkpoint: &mut dyn FnMut(&gpu_mode::Write, ClockOrigin, &str) -> Result<()>,
        permit: &mut dyn FnMut() -> Result<()>,
    ) -> Result<()> {
        let before = &attempt.before;
        if transaction::capture(
            self,
            sampling,
            &before.plans,
            ClockRead::for_observation(&before.gpu_clocks)?,
        )? != *before
        {
            bail!("模式、风扇或系统电源前态已变化");
        }
        let plan = mode::workflow::checkpoint_for_supply(
            self.hal,
            attempt.target.mode,
            attempt.trigger == ccc_core::oem_supply_mode::Trigger::SupplyChange,
        )?;
        if plan.target != attempt.target {
            bail!("原厂模式联动目标已变化");
        }
        mode::links::Plan::new(
            plan,
            mode::links::Effects {
                node_before: before.gpu_power.clone(),
                node_action: attempt.gpu_action,
                clock_policy: attempt.clock_policy.clone(),
                clock_before: before.gpu_clocks.clone(),
                on_ac: attempt.on_ac,
                trigger: Some(attempt.trigger),
            },
        )?
        .execute(&self.links, checkpoint, permit)
    }
    fn restore(
        &self,
        initial: &Initial,
        sampling: &Target,
        expected: &Snapshot,
        permit: &mut dyn FnMut() -> Result<()>,
    ) -> Result<()> {
        if initial.mode.energy()?.hardware_p_state != expected.mode.energy()?.hardware_p_state {
            bail!("EnergyStar 能力已变化，不能恢复旧固件状态");
        }
        if transaction::capture(
            self,
            sampling,
            &initial.plans,
            ClockRead::for_observation(&expected.gpu_clocks)?,
        )? != *expected
        {
            bail!("恢复预览后的模式、风扇或电源计划已变化");
        }
        self.links
            .preflight_restore(initial.gpu_power.as_ref(), expected.gpu_power.as_ref())?;
        let enabling = initial
            .gpu_power
            .as_ref()
            .is_some_and(|gpu| gpu.target.enabled)
            && expected
                .gpu_power
                .as_ref()
                .is_some_and(|gpu| !gpu.target.enabled);
        if enabling {
            self.links.restore_node(
                initial.gpu_power.as_ref(),
                expected.gpu_power.as_ref(),
                permit,
            )?;
        }
        self.links.restore_clocks(
            initial.gpu_clocks.as_ref(),
            &expected.gpu_clocks,
            enabling,
            expected
                .gpu_power
                .as_ref()
                .is_some_and(|gpu| !gpu.target.enabled),
            permit,
        )?;
        permit()?;
        mode::workflow::restore_snapshot(self.hal, &initial.mode.restore()?)?;
        plans::restore_other_plans(
            self.hal.system_power.as_ref(),
            &initial.plans,
            &expected.plans,
            &initial.mode,
            permit,
        )?;
        if !enabling {
            self.links.restore_node(
                initial.gpu_power.as_ref(),
                expected.gpu_power.as_ref(),
                permit,
            )?;
        }
        Ok(())
    }
}
