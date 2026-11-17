use super::{environment::Environment, request::Request, sequence::Sequence, *};
use crate::{
    gpuctl::GpuController,
    hal::{Hal, authorization::session::Authorized},
};
use std::{cell::RefCell, sync::MutexGuard};

struct Prepared {
    request: Request,
    applied: bool,
    attempted: bool,
}
pub(crate) struct Scope<'a> {
    owner: &'a Controller,
    _lock: MutexGuard<'a, ()>,
    prepared: Vec<Prepared>,
    session: Option<Box<dyn Authorized>>,
    failed: bool,
}
impl Controller {
    pub(crate) fn supported_in_control(&self, hal: &Hal) -> Result<bool> {
        self.environment()?.supported(hal)
    }
    pub(crate) fn list_in_control(&self, hal: &Hal) -> Result<Vec<Device>> {
        self.environment()?.list(hal)
    }
    pub(crate) fn scope(&self) -> Result<Scope<'_>> {
        Ok(Scope {
            owner: self,
            _lock: self
                .mutation
                .lock()
                .map_err(|_| anyhow!("独显控制状态异常"))?,
            prepared: Vec::new(),
            session: None,
            failed: false,
        })
    }
    fn environment(&self) -> Result<&dyn Environment> {
        if let Some(environment) = &self.environment {
            return Ok(environment.as_ref());
        }
        #[cfg(not(test))]
        {
            Ok(&environment::Native)
        }
        #[cfg(test)]
        {
            bail!("隔离独显控制器未注入环境，禁止访问真实设备")
        }
    }
    #[cfg(test)]
    pub(crate) fn with_environment(environment: std::sync::Arc<dyn Environment>) -> Self {
        Self {
            environment: Some(environment),
            ..Self::default()
        }
    }
}
impl Scope<'_> {
    pub fn list(&self, hal: &Hal) -> Result<Vec<Device>> {
        self.owner.list_in_control(hal)
    }
    pub fn read(&self, hal: &Hal) -> Result<Profile> {
        self.owner.read(&Reader {
            hal,
            environment: self.owner.environment()?,
        })
    }
    pub fn preview(&self, hal: &Hal, expected: &Device, target: Target) -> Result<Option<Request>> {
        let current = self
            .list(hal)?
            .into_iter()
            .find(|d| d.id == expected.id)
            .ok_or_else(|| anyhow!("原厂独显控制节点已移除"))?;
        if current != *expected {
            bail!("原厂独显状态或策略已变化，请重新读取并确认");
        }
        let request = Request {
            expected: current,
            target,
            restore: false,
        };
        request.encoded()?;
        Ok((request.expected.target() != target).then_some(request))
    }
    pub fn prepare(&mut self, requests: &[Request]) -> Result<()> {
        if !self.prepared.is_empty() || self.failed {
            bail!("独显授权会话已准备");
        }
        let sequence = Sequence::new(requests)?;
        let environment = self.owner.environment()?;
        self.session = Some(environment.authorize(sequence.clone())?);
        self.prepared = sequence
            .requests
            .into_iter()
            .map(|request| Prepared {
                request,
                applied: false,
                attempted: false,
            })
            .collect();
        Ok(())
    }
    pub(crate) fn next_request(&self) -> Result<Option<&Request>> {
        if self.failed {
            bail!("独显阶段序列已经失败或恢复，请重新预览并准备");
        }
        Ok(self
            .prepared
            .iter()
            .find(|item| !item.attempted && !item.applied)
            .map(|item| &item.request))
    }
    pub(crate) fn attempted(&self) -> bool {
        self.prepared.iter().any(|p| p.attempted)
    }
    pub(crate) fn synchronize(&self, hal: &Hal, clocks: &GpuController) -> Result<()> {
        sync_connections(hal, clocks, &self.list(hal)?)
    }
    pub fn execute(&mut self, hal: &Hal, clocks: &GpuController, request: &Request) -> Result<()> {
        if self.failed {
            bail!("独显阶段序列已经失败或恢复，请重新预览并准备");
        }
        let result = self.execute_next(hal, clocks, request);
        if let Err(error) = result {
            if let Err(recovery) = self.rollback(hal, clocks) {
                bail!("{error:#}；独显本次恢复失败：{recovery:#}");
            }
            return Err(error);
        }
        result
    }
    fn execute_next(&mut self, hal: &Hal, clocks: &GpuController, request: &Request) -> Result<()> {
        let index = self
            .prepared
            .iter()
            .position(|item| !item.attempted && !item.applied)
            .ok_or_else(|| anyhow!("独显授权未就绪或全部阶段已经执行"))?;
        if self.prepared[index].request != *request {
            bail!("独显授权与本次阶段或顺序不一致");
        }
        self.preview(hal, &request.expected, request.target)?;
        let environment = self.owner.environment()?;
        let prepared = &mut self.prepared[index];
        let session = self
            .session
            .as_mut()
            .ok_or_else(|| anyhow!("独显系统授权未就绪"))?;
        let result = with_paused(hal, clocks, environment, || {
            let io = AuthorizedAccess {
                hal,
                environment,
                prepared: RefCell::new(&mut *prepared),
                session: RefCell::new(&mut *session),
            };
            self.owner
                .apply(&io, &request.expected, request.target, request.restore)
        });
        result?;
        self.prepared[index].applied = true;
        Ok(())
    }
    pub fn rollback(&mut self, hal: &Hal, clocks: &GpuController) -> Result<()> {
        self.failed = true;
        let environment = self.owner.environment()?;
        let Some(session) = self.session.as_mut().filter(|session| session.dirty()) else {
            return Ok(());
        };
        with_paused(hal, clocks, environment, || session.rollback())?;
        for prepared in &mut self.prepared {
            prepared.applied = false;
        }
        Ok(())
    }
    pub fn finish(&mut self) -> Result<()> {
        self.session
            .take()
            .map(|session| session.finish())
            .unwrap_or(Ok(()))
    }
}
impl Drop for Scope<'_> {
    fn drop(&mut self) {
        drop(self.session.take());
    }
}
struct Reader<'a> {
    hal: &'a Hal,
    environment: &'a dyn Environment,
}
impl Access for Reader<'_> {
    fn list(&self) -> Result<Vec<Device>> {
        self.environment.list(self.hal)
    }
    fn set(&self, _: &Device, _: Target) -> Result<()> {
        bail!("只读独显环境没有执行许可")
    }
}
struct AuthorizedAccess<'a> {
    hal: &'a Hal,
    environment: &'a dyn Environment,
    prepared: RefCell<&'a mut Prepared>,
    session: RefCell<&'a mut Box<dyn Authorized>>,
}
impl Access for AuthorizedAccess<'_> {
    fn list(&self) -> Result<Vec<Device>> {
        self.environment.list(self.hal)
    }
    fn set(&self, expected: &Device, target: Target) -> Result<()> {
        let mut prepared = self.prepared.borrow_mut();
        if prepared.request.expected != *expected || prepared.request.target != target {
            bail!("独显执行许可与请求不一致");
        }
        prepared.attempted = true;
        self.session.borrow_mut().execute()
    }
}
pub(super) fn with_paused(
    hal: &Hal,
    clocks: &GpuController,
    environment: &dyn Environment,
    action: impl FnOnce() -> Result<()>,
) -> Result<()> {
    let previous = hal.gpu_suspended();
    clocks.set_power_suspended(true)?;
    if let Err(error) = hal.suspend_gpu(true) {
        clocks.set_power_suspended(previous)?;
        return Err(error);
    }
    let result = action();
    let reset = environment.list(hal).and_then(|devices| {
        if devices.len() != 1 || !devices[0].healthy() {
            bail!("无法确认独显控制状态，监控与调校保持暂停");
        }
        sync_connections(hal, clocks, &devices)
    });
    match (result, reset) {
        (Err(error), Err(reset)) => bail!("{error:#}；GPU连接重置失败：{reset:#}"),
        (Err(error), _) => Err(error),
        (_, result) => result,
    }
}
pub(super) fn sync_connections(
    hal: &Hal,
    clocks: &GpuController,
    devices: &[Device],
) -> Result<()> {
    if let [device] = devices {
        if !device.healthy() {
            bail!("原厂独显节点状态异常，无法恢复GPU通道");
        }
        hal.suspend_gpu(!device.enabled)?;
        clocks.set_power_suspended(!device.enabled)?;
    }
    Ok(())
}
