use super::*;
use crate::gpu_power::{
    mode::State,
    profile::{Review, Stages},
};
use powerctl::mode::{links, profile as mode_profile};

pub(super) struct Plan {
    pub power: Option<Review>,
    pub deferred: bool,
    source_offsets: Option<GpuCheckpoint>,
}
impl Plan {
    pub fn prepare(
        hal: &Hal,
        gpu: &GpuController,
        devices: &Scopes<'_>,
        settings: &ProfileSettings,
        mode: Option<&links::Plan>,
        initial: Option<&Self>,
    ) -> Result<Self> {
        let power = match initial {
            Some(initial) => initial.power.clone(),
            None => settings
                .gpu_power
                .as_ref()
                .map(|target| devices.mode.node_scope().preview_profile(hal, target))
                .transpose()?,
        };
        let mut plan = Self {
            power,
            deferred: false,
            source_offsets: None,
        };
        let stages = plan.stages(mode)?;
        if let Some(stages) = &stages {
            let actual = devices.mode.context.device()?;
            let expected = if initial.is_some() {
                stages.mode_before()
            } else {
                stages.before.clone()
            };
            if actual.as_ref() != Some(&expected) {
                bail!("档案独显阶段前态或显示用途已变化");
            }
            plan.deferred = !expected.enabled && stages.preparation.is_some();
        }
        if plan.power.is_some() || mode.is_some() {
            devices.mode.node_scope().synchronize(hal, gpu)?;
        }
        let offsets = if plan.deferred {
            None
        } else {
            offsets_preflight(gpu, settings)?
        };
        plan.source_offsets = if plan.power.is_some() && !hal.gpu_suspended() {
            if settings.gpu.is_some() {
                Some(gpu.checkpoint()?)
            } else {
                gpu.checkpoint().ok()
            }
        } else {
            offsets
        };
        if let Some(original) = initial
            .filter(|_| devices.mode.node_scope().attempted())
            .and_then(|plan| plan.source_offsets)
        {
            plan.source_offsets = Some(original);
        }
        Ok(plan)
    }
    pub fn stages(&self, mode: Option<&links::Plan>) -> Result<Option<Stages>> {
        match mode {
            Some(mode) => Ok(mode_profile::stages(mode)?.cloned()),
            None => self
                .power
                .as_ref()
                .map(Review::independent_stages)
                .transpose(),
        }
    }
    pub fn needs_fan_snapshot(&self) -> bool {
        self.power
            .as_ref()
            .is_some_and(|r| !r.target.enabled && r.software_fan_running)
    }
    pub fn authorize(&self, devices: &Scopes<'_>, mode: Option<&links::Plan>) -> Result<()> {
        if let Some(stages) = self.stages(mode)? {
            devices.mode.freeze_requests(stages.requests())?;
            devices.mode.authorize()?;
        }
        Ok(())
    }
    pub fn enable(
        &self,
        devices: &Scopes<'_>,
        mode: Option<&links::Plan>,
        permit: &mut dyn FnMut() -> Result<()>,
    ) -> Result<()> {
        if let Some(mode) = mode {
            return mode_profile::prepare(&devices.mode, mode, permit);
        }
        if let Some(request) = self.stages(mode)?.and_then(|graph| graph.preparation) {
            devices.mode.perform(
                &State::from_device(&request.expected)?,
                request.target,
                permit,
            )?;
        }
        Ok(())
    }
    pub fn finish(
        &self,
        devices: &Scopes<'_>,
        mode: Option<&links::Plan>,
        enabled: bool,
        permit: &mut dyn FnMut() -> Result<()>,
    ) -> Result<()> {
        if let Some(mode) = mode {
            return mode_profile::finish(&devices.mode, mode, enabled, permit);
        }
        if let Some(request) = self
            .stages(mode)?
            .and_then(|graph| graph.finish)
            .filter(|request| request.target.enabled == enabled)
        {
            devices.mode.perform(
                &State::from_device(&request.expected)?,
                request.target,
                permit,
            )?;
        }
        Ok(())
    }
    pub fn offsets(
        &self,
        gpu: &GpuController,
        settings: &ProfileSettings,
        touched: &mut bool,
        steps: &mut Vec<ApplyStep>,
    ) -> Result<()> {
        if let Some(wanted) = settings.gpu {
            *touched = true;
            let outcome = gpu.apply_settings(&wanted);
            let result = if outcome.ok {
                Ok(())
            } else {
                Err(anyhow::anyhow!(
                    [Some(outcome.message), outcome.rollback_error]
                        .into_iter()
                        .flatten()
                        .collect::<Vec<_>>()
                        .join("；")
                ))
            };
            stage(steps, "GPU 偏移", result)?;
        }
        Ok(())
    }
    pub fn close(
        &self,
        hal: &Hal,
        devices: &Scopes<'_>,
        mode: Option<&links::Plan>,
        fan_touched: &mut bool,
        steps: &mut Vec<ApplyStep>,
        permit: &mut dyn FnMut() -> Result<()>,
    ) -> Result<()> {
        if !self
            .stages(mode)?
            .and_then(|graph| graph.finish)
            .is_some_and(|request| !request.target.enabled)
        {
            return Ok(());
        }
        permit()?;
        if fanctl::software::active(hal)? {
            *fan_touched = true;
            stage(steps, "退出软件风扇", fanctl::software::stop_safely(hal))?;
        }
        stage(
            steps,
            "独显节点与自动关断策略",
            self.finish(devices, mode, false, permit),
        )
    }
    pub fn recheck(
        &self,
        hal: &Hal,
        devices: &Scopes<'_>,
        mode: Option<&links::Plan>,
    ) -> Result<()> {
        if let Some(graph) = self.stages(mode)? {
            if devices.mode.context.device()?.as_ref() != Some(&graph.final_device()) {
                bail!("档案独显最终状态或显示用途已变化");
            }
            if !graph.final_device().enabled && fanctl::software::active(hal)? {
                bail!("独显关闭后，软件调速仍在运行或尚未确认退出");
            }
        }
        Ok(())
    }
    pub fn rollback(
        &self,
        gpu: &GpuController,
        devices: &mut Scopes<'_>,
        offsets_touched: bool,
        steps: &mut Vec<ApplyStep>,
    ) {
        let power_touched = devices.mode.node_scope().attempted();
        if power_touched {
            let _ = stage(steps, "恢复本次独显状态", devices.mode.rollback_nodes());
        }
        if power_touched || offsets_touched {
            if let Some(before) = &self.source_offsets {
                let _ = stage(
                    steps,
                    "恢复 GPU",
                    gpu.restore_confirmed_checkpoint_in_control(before, &mut || {
                        devices.mode.context.hal.ensure_mode_change_allowed()?;
                        crate::hal::oem_owner::ensure_owner_absent()
                    }),
                );
            }
        }
    }
}
fn offsets_preflight(
    gpu: &GpuController,
    settings: &ProfileSettings,
) -> Result<Option<GpuCheckpoint>> {
    let Some(wanted) = settings.gpu else {
        return Ok(None);
    };
    let current = gpu.read_profile();
    if !current.available {
        bail!(
            "GPU偏移不可用：{}",
            current
                .unavailable_reason
                .as_deref()
                .unwrap_or("驱动未确认可用")
        );
    }
    for (want, editable, min, max) in [
        (
            wanted.core_offset_mhz,
            current.core_editable,
            current.core_min_mhz,
            current.core_max_mhz,
        ),
        (
            wanted.mem_offset_mhz,
            current.mem_editable,
            current.mem_min_mhz,
            current.mem_max_mhz,
        ),
    ] {
        if want.is_some_and(|v| !editable || v < min || v > max) {
            bail!("GPU 偏移不可写或超出驱动范围");
        }
    }
    let mut before = gpu.checkpoint()?;
    if wanted.core_offset_mhz.is_none() {
        before.core_khz = None;
    }
    if wanted.mem_offset_mhz.is_none() {
        before.mem_khz = None;
    }
    Ok(Some(before))
}
