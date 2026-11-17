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
    pub fn restore_runtime(
        &self,
        hal: &Hal,
        clocks: &GpuController,
        display: &display::Controller,
    ) -> Result<()> {
        let mut scope = self.scope()?;
        let request = {
            let _gate = hal.begin_write()?;
            available(hal, display, false)?;
            let baseline = self
                .baseline
                .lock()
                .map_err(|_| anyhow!("独显恢复状态异常"))?;
            if baseline.is_empty() {
                bail!("没有原厂独显初始快照");
            }
            let devices = scope.list(hal)?;
            if baseline.len() != 1 {
                bail!("原厂独显实例已变化，请核对初始恢复记录");
            }
            let (id, target) = baseline.first_key_value().unwrap();
            let current = devices
                .iter()
                .find(|device| &device.id == id)
                .ok_or_else(|| anyhow!("原厂独显控制节点已移除，拒绝恢复其他实例"))?;
            let request = request::Request {
                expected: current.clone(),
                target: *target,
                restore: true,
            };
            request.encoded()?;
            request
        };
        scope.prepare(std::slice::from_ref(&request))?;
        let result = (|| {
            let _gate = hal.begin_write()?;
            available(hal, display, false)?;
            scope.execute(hal, clocks, &request)
        })();
        finish(result, scope.finish())
    }
}
fn available(hal: &Hal, display: &display::Controller, allow_software: bool) -> Result<()> {
    if display.trial_pending()? {
        bail!("请先保留或恢复刷新率试用，再请求独显系统授权");
    }
    if !allow_software && crate::fanctl::software::read(hal)?.is_some() {
        bail!("请先退出软件风扇曲线，再请求独显系统授权");
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
