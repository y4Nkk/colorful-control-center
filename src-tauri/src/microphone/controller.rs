use super::*;

#[derive(Default)]
pub struct Controller {
    mutation: Mutex<()>,
    #[cfg(test)]
    test_io: Option<std::sync::Arc<dyn Io + Send + Sync>>,
}
pub(crate) struct Scope<'a> {
    controller: &'a Controller,
    _lock: MutexGuard<'a, ()>,
}
impl Controller {
    #[cfg(test)]
    pub(crate) fn with_test_io(mut self, io: std::sync::Arc<dyn Io + Send + Sync>) -> Self {
        self.test_io = Some(io);
        self
    }
    pub(crate) fn scope(&self) -> Result<Scope<'_>> {
        Ok(Scope {
            controller: self,
            _lock: self
                .mutation
                .lock()
                .map_err(|_| anyhow!("麦克风控制状态异常"))?,
        })
    }
    fn with_io<T>(&self, hal: &Hal, action: impl FnOnce(&dyn Io) -> Result<T>) -> Result<T> {
        #[cfg(test)]
        {
            let _ = hal;
            action(
                self.test_io
                    .as_ref()
                    .context("隔离麦克风传输未配置，禁止回退原生接口")?
                    .as_ref(),
            )
        }
        #[cfg(not(test))]
        {
            action(&native::Native(hal))
        }
    }
    pub fn read(&self, hal: &Hal) -> Result<Profile> {
        let scope = self.scope()?;
        let _gate = hal.begin_write()?;
        scope.read(hal)
    }
    pub fn apply(&self, hal: &Hal, request: &Request) -> Result<()> {
        let scope = self.scope()?;
        let _gate = hal.begin_write()?;
        let profile = scope.read(hal)?;
        if profile.revision != request.expected_revision {
            bail!("麦克风预览已变化，请重新确认");
        }
        let mut endpoints = profile.current.endpoints.clone();
        for endpoint in &mut endpoints {
            endpoint.muted = request.muted;
        }
        let plan = scope.prepare(
            hal,
            &Target {
                endpoints,
                saved_muted: request.muted,
            },
        )?;
        if plan.before != profile.current {
            bail!("麦克风在准备期间变化，请重新预览");
        }
        scope.execute(hal, &plan, &mut false)
    }
    pub(crate) fn hotkey_identity(&self, hal: &Hal) -> Result<String> {
        let scope = self.scope()?;
        let _gate = hal.begin_write()?;
        let profile = scope.read(hal)?;
        if let Some(reason) = profile.blocked_reason {
            bail!("{reason}");
        }
        profile.current.identity()
    }
    pub(crate) fn toggle(&self, hal: &Hal, expected_identity: &str) -> Result<bool> {
        let scope = self.scope()?;
        let _gate = hal.begin_write()?;
        let current = scope.read(hal)?.current;
        if current.identity()? != expected_identity {
            bail!("麦克风设备集合已变化，请重新确认热键");
        }
        let muted = !current.endpoints.iter().all(|endpoint| endpoint.muted);
        let mut endpoints = current.endpoints.clone();
        for endpoint in &mut endpoints {
            endpoint.muted = muted;
        }
        let plan = scope.prepare(
            hal,
            &Target {
                endpoints,
                saved_muted: muted,
            },
        )?;
        if plan.before != current {
            bail!("麦克风在准备期间变化，未执行旧状态的热键");
        }
        scope.execute(hal, &plan, &mut false)?;
        Ok(muted)
    }
}
impl Scope<'_> {
    pub(crate) fn read(&self, hal: &Hal) -> Result<Profile> {
        self.controller.with_io(hal, |io| {
            let current = transaction::capture(io)?;
            let blocked_reason = if current.endpoints.is_empty() {
                Some("没有活动麦克风".into())
            } else {
                io.ensure_available().err().map(|e| format!("{e:#}"))
            };
            Ok(Profile {
                revision: current.revision()?,
                current,
                blocked_reason,
            })
        })
    }
    pub(crate) fn capture(&self, hal: &Hal) -> Result<Option<Target>> {
        self.controller.with_io(hal, |io| {
            let current = transaction::capture(io)?;
            if current.endpoints.is_empty() {
                return Ok(None);
            }
            if current.saved > 1 {
                bail!("原厂静音偏好未知，无法捕获可执行档案");
            }
            Ok(Some(Target {
                endpoints: current.endpoints,
                saved_muted: current.saved != 0,
            }))
        })
    }
    pub(crate) fn prepare(&self, hal: &Hal, target: &Target) -> Result<Plan> {
        self.controller.with_io(hal, |io| {
            let before = transaction::capture(io)?;
            let after = before.target(target)?;
            if before != after {
                io.ensure_available()?;
            }
            transaction::prepare(before, after)
        })
    }
    pub(crate) fn execute(
        &self,
        hal: &Hal,
        plan: &Plan,
        needs_rollback: &mut bool,
    ) -> Result<()> {
        self.controller
            .with_io(hal, |io| transaction::execute(io, plan, needs_rollback))
    }
    pub(crate) fn rollback(&self, hal: &Hal, plan: &Plan) -> Result<()> {
        self.controller
            .with_io(hal, |io| transaction::rollback(io, plan))
    }
}
