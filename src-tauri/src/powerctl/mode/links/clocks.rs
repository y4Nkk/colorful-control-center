use super::{PreparedClocks, Session, clocks};
use anyhow::{Context, Result, bail};
use clocks::{ClockBaseline, ClockObservation, ClockOrigin};

impl Session<'_> {
    pub fn read_clocks(&self) -> Result<Option<clocks::RawState>> {
        if self.context.device()?.is_some_and(|gpu| !gpu.enabled) {
            return Ok(None);
        }
        self.context.clocks.read_mode_gpu_in_control().map(Some)
    }
    pub fn prepare_clocks(
        &self,
        policy: &clocks::Policy,
        mode: u8,
        on_ac: bool,
        resume: bool,
        before: &ClockObservation,
        origin: Option<ClockOrigin>,
        checkpoint: &mut dyn FnMut(&clocks::Write, ClockOrigin, &str) -> Result<()>,
        permit: &mut dyn FnMut() -> Result<()>,
    ) -> Result<()> {
        if self.clock_prepared_once.replace(true) {
            bail!("模式GPU频率已经准备，不能替换写前快照");
        }
        let value = self.context.clocks.prepare_mode_gpu_in_control(
            self.context.source.as_ref(),
            policy,
            mode,
            on_ac,
            resume,
            permit,
        )?;
        if let Some(value) = value {
            if before
                .raw()
                .is_some_and(|expected| expected != &value.write.before)
            {
                bail!("GPU频率准备前态与模式预览不一致");
            }
            let origin = origin.context("模式GPU频率的实际读取来源缺失")?;
            checkpoint(&value.write, origin, value.driver_identity())?;
            *self.clock_prepared.borrow_mut() = Some(PreparedClocks { value, origin });
        }
        Ok(())
    }
    pub fn apply_clocks(
        &self,
        policy: &mut dyn FnMut() -> Result<clocks::Policy>,
        checkpoint: &mut dyn FnMut(&clocks::Write, ClockOrigin, &str) -> Result<()>,
        permit: &mut dyn FnMut() -> Result<()>,
    ) -> Result<()> {
        let prepared = self.clock_prepared.borrow();
        let Some(prepared) = prepared.as_ref() else {
            return Ok(());
        };
        let policy = policy()?;
        self.clock_started.set(true);
        self.context.clocks.apply_mode_gpu_in_control(
            self.context.source.as_ref(),
            &policy,
            &prepared.value,
            &mut |write| checkpoint(write, prepared.origin, prepared.value.driver_identity()),
            permit,
        )
    }
    pub fn recover_clocks(&self) -> Result<()> {
        if !self.clock_started.get() {
            return Ok(());
        }
        let prepared = self.clock_prepared.borrow();
        let Some(prepared) = prepared.as_ref() else {
            return Ok(());
        };
        let mut permit = || {
            self.context.hal.ensure_mode_change_allowed()?;
            crate::hal::oem_owner::ensure_owner_absent()
        };
        let actual = self
            .context
            .clocks
            .read_confirmed_mode_gpu_in_control(&mut permit)?;
        if !prepared.value.write.may_recover(&actual) {
            bail!("GPU频率在失败后被其他操作更改，保留恢复记录");
        }
        self.context.clocks.restore_mode_gpu_in_control(
            &prepared.value.write.before,
            &actual,
            &mut permit,
        )?;
        self.clock_started.set(false);
        Ok(())
    }
    pub fn restore_clocks(
        &self,
        baseline: Option<&ClockBaseline>,
        expected: &ClockObservation,
        enabled_now: bool,
        was_off: bool,
        permit: &mut dyn FnMut() -> Result<()>,
    ) -> Result<()> {
        let Some(baseline) = baseline else {
            return Ok(());
        };
        let actual = self
            .context
            .clocks
            .read_confirmed_mode_gpu_in_control(permit)?;
        if !enabled_now && !was_off && expected.raw() != Some(&actual) {
            bail!("GPU频率恢复预览已变化");
        }
        self.context
            .clocks
            .restore_mode_gpu_in_control(&baseline.state, &actual, permit)
    }
}
