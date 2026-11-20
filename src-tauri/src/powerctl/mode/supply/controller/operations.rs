use super::Controller;
use crate::{
    display, gpu_power,
    gpuctl::GpuController,
    hal::{Hal, oem_owner},
    powerctl::mode::supply::{
        control::source,
        native::Native,
        scheduler::Work,
        transaction::{self, Applied},
    },
};
use anyhow::{Result, anyhow, bail};

impl Controller {
    pub fn read(
        &self,
        hal: &Hal,
        clocks: &GpuController,
        power: &gpu_power::Controller,
    ) -> Result<super::Status> {
        let _operation = self.lock()?;
        let io = Native::new(hal, clocks, power)?;
        let _gate = hal.begin_write()?;
        let mut status = self.read_in_control(&io)?;
        let previous_owner_issue = status
            .issue
            .as_deref()
            .is_some_and(oem_owner::is_blocked_reason);
        if status.view.can_enable
            || status.can_resume
            || status.view.can_restore
            || previous_owner_issue
        {
            status.blocked_reason = oem_owner::blocked_reason();
            let writable = status.blocked_reason.is_none();
            status.view.can_enable &= writable;
            status.can_resume &= writable;
            status.view.can_restore &= writable;
        }
        if previous_owner_issue {
            status.issue = None;
        }
        Ok(status)
    }
    pub fn enable(
        &self,
        hal: &Hal,
        clocks: &GpuController,
        power: &gpu_power::Controller,
        display: &display::Controller,
        expected: &str,
    ) -> Result<()> {
        let _operation = self.lock()?;
        let io = Native::new(hal, clocks, power)?;
        let _gate = hal.begin_write()?;
        self.enable_in_control(&io, expected, display::now_ms(), &mut || {
            self.preflight(hal, display)
        })
    }
    pub fn resume(
        &self,
        hal: &Hal,
        clocks: &GpuController,
        power: &gpu_power::Controller,
        display: &display::Controller,
        expected: &str,
    ) -> Result<()> {
        let _operation = self.lock()?;
        let io = Native::new(hal, clocks, power)?;
        let _gate = hal.begin_write()?;
        let view = self.read_in_control(&io)?;
        if view.view.revision != expected || !view.can_resume {
            bail!("供电模式继续预览已变化或不可继续");
        }
        self.resume_in_control(&io, display::now_ms(), &mut || self.preflight(hal, display))
    }
    pub fn restore(
        &self,
        hal: &Hal,
        clocks: &GpuController,
        power: &gpu_power::Controller,
        display: &display::Controller,
        expected: &str,
    ) -> Result<()> {
        let _operation = self.lock()?;
        let io = Native::new(hal, clocks, power)?;
        let gate = hal.begin_write()?;
        let view = self.read_in_control(&io)?.view;
        if view.revision != expected || !view.can_restore {
            bail!("供电模式恢复预览已变化或不可恢复");
        }
        self.shared.pause(None)?;
        let epoch = self.shared.lock()?.scheduler.ticket();
        let record = self
            .store
            .load()?
            .ok_or_else(|| anyhow!("供电模式记录缺失"))?;
        self.preflight(hal, display)?;
        if let Some((initial, actual)) = record.initial.as_ref().zip(view.current.as_ref()) {
            io.freeze_restore(initial.gpu_power.as_ref(), actual.gpu_power.as_ref())?;
        }
        drop(gate);
        let result = (|| {
            self.shared.permit_restore(epoch)?;
            io.authorize()?;
            let _gate = hal.begin_write()?;
            transaction::restore(&io, &self.store, expected, &mut || {
                self.shared.permit_restore(epoch)?;
                self.preflight(hal, display)
            })
        })();
        finish(result, io.finish())
    }
    pub(in crate::powerctl::mode::supply) fn execute_work(
        &self,
        hal: &Hal,
        clocks: &GpuController,
        power: &gpu_power::Controller,
        display: &display::Controller,
        refresh: &display::automatic::Controller,
        work: Work,
    ) -> Result<Option<Applied>> {
        let _operation = self.lock()?;
        let io = Native::new(hal, clocks, power)?;
        let mut confirmed = None;
        let result = (|| {
            let gate = hal.begin_write()?;
            let (revision, review) = self.preview_work(&io, work)?;
            self.preflight(hal, display)?;
            io.freeze(&review, work.trigger)?;
            drop(gate);
            self.shared.permit(work.epoch, work.source)?;
            io.authorize()?;
            let _gate = hal.begin_write()?;
            let applied = self.execute_in_control(&io, work, &revision, &mut || {
                self.preflight(hal, display)?;
                if source(crate::battery::system_power()?.ac_online) != work.source {
                    bail!("实际供电已变化，停止原厂模式目标");
                }
                Ok(())
            })?;
            confirmed = applied;
            if let Some(applied) = applied {
                refresh.mode_completed(applied, review.dc_mode_power)?;
            }
            Ok(applied)
        })();
        let result = finish(result, io.finish());
        self.shared.report(
            work.trigger,
            confirmed,
            result.as_ref().err().map(|e| format!("{e:#}")),
            display::now_ms(),
        )?;
        result
    }
    pub(in crate::powerctl::mode::supply) fn resume_saved(
        &self,
        hal: &Hal,
        clocks: &GpuController,
        power: &gpu_power::Controller,
        display: &display::Controller,
    ) -> Result<()> {
        let Some(epoch) = self.shared.claim_startup()? else {
            return Ok(());
        };
        let _operation = self.lock()?;
        if self.store.load()?.is_none_or(|record| !record.enabled) {
            return Ok(());
        }
        let io = Native::new(hal, clocks, power)?;
        let _gate = hal.begin_write()?;
        self.resume_in_control(&io, display::now_ms(), &mut || {
            self.shared.permit_setup(epoch)?;
            self.preflight(hal, display)
        })
    }
    fn preflight(&self, hal: &Hal, display: &display::Controller) -> Result<()> {
        crate::profiles::policy::ensure_inactive(&self.profiles_path)?;
        hal.ensure_mode_change_allowed()?;
        oem_owner::ensure_owner_absent()?;
        if display.trial_pending()? {
            bail!("先结束当前显示试用，再执行原厂模式联动");
        }
        Ok(())
    }
}
fn finish<T>(result: Result<T>, closed: Result<()>) -> Result<T> {
    match (result, closed) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(error), Ok(())) => Err(error),
        (Ok(_), Err(error)) => bail!("目标事务已完成，独显授权会话结束失败：{error:#}"),
        (Err(error), Err(closed)) => bail!("{error:#}；独显授权会话结束失败：{closed:#}"),
    }
}
