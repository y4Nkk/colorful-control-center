use super::Session;
use crate::gpu_power::{
    Device, Target, mode::State, operation, request::Request, sequence::Sequence,
};
use anyhow::{Result, anyhow, bail};

impl Session<'_> {
    pub fn review_nodes(&self) -> Result<()> {
        let next = self.gpu.borrow().next_request()?.cloned();
        if let Some(request) = next {
            if self.context.device()?.as_ref() != Some(&request.expected) {
                bail!("独显阶段前态或显示用途已变化，未执行关联输出");
            }
        }
        Ok(())
    }
    pub fn freeze_requests(&self, requests: Vec<Request>) -> Result<()> {
        if self.authorized.get() || self.pending.borrow().is_some() {
            bail!("模式独显请求已经冻结或授权");
        }
        if !requests.is_empty() {
            Sequence::new(&requests)?;
        }
        *self.pending.borrow_mut() = Some(requests);
        Ok(())
    }
    pub fn freeze_node(&self, current: &Device, target: Target) -> Result<()> {
        let request = self
            .gpu
            .borrow()
            .preview(self.context.hal, current, target)?;
        self.freeze_requests(request.into_iter().collect())
    }
    pub fn authorize(&self) -> Result<()> {
        if self.authorized.get() {
            bail!("模式独显序列已经授权");
        }
        if let Some(requests) = self.pending.borrow().as_ref().filter(|r| !r.is_empty()) {
            self.gpu.borrow_mut().prepare(requests)?;
        }
        self.pending.borrow_mut().take();
        self.authorized.set(true);
        Ok(())
    }
    pub fn perform(
        &self,
        before: &State,
        target: Target,
        permit: &mut dyn FnMut() -> Result<()>,
    ) -> Result<()> {
        let current = self
            .context
            .device()?
            .ok_or_else(|| anyhow!("独显节点不可用"))?;
        if State::from_device(&current)? != *before {
            bail!("独显前态已变化");
        }
        let request = self.gpu.borrow().next_request()?.cloned();
        if before.target == target && !request.as_ref().is_some_and(|request| request.restore) {
            return Ok(());
        }
        let request = request.ok_or_else(|| anyhow!("独显系统授权未就绪"))?;
        if request.expected.id != before.id
            || request.expected.target() != before.target
            || request.target != target
        {
            bail!("独显授权与本次模式目标不一致");
        }
        permit()?;
        self.gpu
            .borrow_mut()
            .execute(self.context.hal, self.context.clocks, &request)
    }
    pub fn preflight_restore(
        &self,
        initial: Option<&State>,
        expected: Option<&State>,
    ) -> Result<()> {
        match (initial, expected, self.context.device()?) {
            (None, None, None) => Ok(()),
            (Some(initial), Some(expected), Some(current))
                if initial.id == expected.id && State::from_device(&current)? == *expected =>
            {
                operation::preflight(&current, expected.target, initial.target)
            }
            _ => bail!("独显节点或策略已变化，无法恢复旧目标"),
        }
    }
    pub fn freeze_restore(&self, initial: Option<&State>, expected: Option<&State>) -> Result<()> {
        self.preflight_restore(initial, expected)?;
        if let (Some(initial), Some(_)) = (initial, expected) {
            let device = self
                .context
                .device()?
                .ok_or_else(|| anyhow!("独显节点不可用"))?;
            self.freeze_requests(vec![Request {
                expected: device,
                target: initial.target,
                restore: true,
            }])?;
        }
        Ok(())
    }
    pub fn restore_node(
        &self,
        initial: Option<&State>,
        expected: Option<&State>,
        permit: &mut dyn FnMut() -> Result<()>,
    ) -> Result<()> {
        match (initial, expected) {
            (None, None) => Ok(()),
            (Some(initial), Some(expected)) if initial.id == expected.id => {
                self.perform(expected, initial.target, permit)
            }
            _ => bail!("独显恢复设备不一致"),
        }
    }
    pub fn rollback_nodes(&self) -> Result<()> {
        self.gpu
            .borrow_mut()
            .rollback(self.context.hal, self.context.clocks)
    }
    pub fn finish(&self) -> Result<()> {
        self.pending.borrow_mut().take();
        self.gpu.borrow_mut().finish()
    }
}
