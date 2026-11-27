use super::*;
use std::sync::MutexGuard;

pub(crate) struct Scope<'a> {
    pub(super) controller: &'a Controller,
    _lock: MutexGuard<'a, ()>,
}
impl Controller {
    pub(crate) fn scope(&self) -> Result<Scope<'_>> {
        Ok(Scope {
            controller: self,
            _lock: self
                .mutation
                .lock()
                .map_err(|_| anyhow!("充电控制状态异常"))?,
        })
    }
    pub(super) fn with_io<T>(
        &self,
        hal: &Hal,
        f: impl FnOnce(&dyn Io, &dyn energy::Io) -> Result<T>,
    ) -> Result<T> {
        #[cfg(test)]
        {
            let _ = hal;
            f(
                self.io.as_deref().context("隔离电池控制器未注入充电IO")?,
                self.energy_io
                    .as_deref()
                    .context("隔离电池控制器未注入节能IO")?,
            )
        }
        #[cfg(not(test))]
        f(&native::Native { hal }, &energy::native::Native { hal })
    }
    #[cfg(test)]
    pub(crate) fn with_test_io(
        mut self,
        io: std::sync::Arc<dyn Io + Send + Sync>,
        energy: std::sync::Arc<dyn energy::Io + Send + Sync>,
    ) -> Self {
        self.io = Some(io);
        self.energy_io = Some(energy);
        self
    }
}
impl Scope<'_> {
    pub(super) fn ensure_available(&self) -> Result<()> {
        if let Some(reason) = calibration::charge_lock(&self.controller.energy_path)? {
            bail!(reason);
        }
        Ok(())
    }
    pub(super) fn charge_lock(&self, io: &dyn energy::Io) -> Result<Option<&'static str>> {
        if let Some(reason) = energy::charge_lock(&self.controller.energy_path)? {
            return Ok(Some(reason));
        }
        let store = energy::FileStore(self.controller.energy_path.clone());
        Ok(energy::observe(io, &store)?
            .and_then(|(snapshot, _)| snapshot.policy)
            .filter(|policy| policy.enabled)
            .map(|_| "节能计划管理充电阈值，请先停止节能计划"))
    }
    pub(super) fn ensure_charge_available(&self, io: &dyn energy::Io) -> Result<()> {
        self.ensure_available()?;
        if let Some(reason) = self.charge_lock(io)? {
            bail!(reason);
        }
        Ok(())
    }
}
