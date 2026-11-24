use super::*;
use crate::{display, gpuctl::GpuController, hal::Hal};

impl Controller {
    pub fn read_runtime(&self, hal: &Hal, clocks: &GpuController) -> Result<Profile> {
        let scope = self.scope()?;
        let _gate = hal.begin_write()?;
        let profile = scope.read(hal)?;
        scope::sync_connections(hal, clocks, &profile.devices)?;
        Ok(profile)
    }
    pub fn apply_runtime(
        &self,
        hal: &Hal,
        clocks: &GpuController,
        display: &display::Controller,
        expected: &Device,
        enabled: bool,
    ) -> Result<()> {
        let mut scope = self.scope()?;
        let request = {
            let _gate = hal.begin_write()?;
            available(hal, display, enabled)?;
            let request = scope.preview(hal, expected, Target::requested(enabled))?;
            if request.is_none() {
                scope::sync_connections(hal, clocks, std::slice::from_ref(expected))?;
            }
            request
        };
        let Some(request) = request else {
            return Ok(());
        };
        scope.prepare(std::slice::from_ref(&request))?;
        let result = (|| {
            let _gate = hal.begin_write()?;
            available(hal, display, enabled)?;
            scope.execute(hal, clocks, &request)
        })();
        finish(result, scope.finish())
    }
}
fn available(hal: &Hal, display: &display::Controller, allow_software: bool) -> Result<()> {
    if display.trial_pending()? {
        bail!("请先保留或恢复刷新率试用，再请求独显系统授权");
    }
    if !allow_software && crate::fanctl::software::active(hal)? {
        bail!("请先确认软件风扇已归还，再请求独显系统授权");
    }
    Ok(())
}
fn finish(result: Result<()>, cleanup: Result<()>) -> Result<()> {
    match (result, cleanup) {
        (Err(error), Err(cleanup)) => bail!("{error:#}；独显授权收尾失败：{cleanup:#}"),
        (Err(error), _) => Err(error),
        (_, result) => result,
    }
}
