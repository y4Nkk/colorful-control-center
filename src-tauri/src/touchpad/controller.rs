use super::*;

#[derive(Default)]
pub struct Controller {
    initial: Mutex<Option<Snapshot>>,
    #[cfg(test)]
    test_io: Option<std::sync::Arc<dyn Io + Send + Sync>>,
}
pub(crate) struct Scope<'a> {
    controller: &'a Controller,
    initial: MutexGuard<'a, Option<Snapshot>>,
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
            initial: self
                .initial
                .lock()
                .map_err(|_| anyhow!("触摸板控制状态异常"))?,
        })
    }
    fn with_io<T>(&self, hal: &Hal, action: impl FnOnce(&dyn Io) -> Result<T>) -> Result<T> {
        #[cfg(test)]
        {
            let _ = hal;
            action(
                self.test_io
                    .as_ref()
                    .context("隔离触摸板传输未配置，禁止回退原生接口")?
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
        let mut scope = self.scope()?;
        let _gate = hal.begin_write()?;
        let profile = scope.read(hal)?;
        if profile.revision != request.expected_revision {
            bail!("触摸板预览已变化，请重新确认");
        }
        let plan = scope.prepare(
            hal,
            &Target {
                devices: profile.current.devices.clone(),
                enabled: request.enabled,
                saved_enabled: request.enabled,
            },
        )?;
        if plan.before != profile.current {
            bail!("触摸板在准备期间变化，请重新预览");
        }
        scope.execute(hal, &plan, &mut false)
    }
    pub fn restore(&self, hal: &Hal, expected_revision: &str) -> Result<()> {
        let mut scope = self.scope()?;
        let _gate = hal.begin_write()?;
        let profile = scope.read(hal)?;
        if profile.revision != expected_revision {
            bail!("触摸板恢复预览已变化，请重新读取");
        }
        let initial = scope
            .initial
            .as_ref()
            .context("没有触摸板初始状态")?
            .clone();
        let plan = transaction::prepare(profile.current, initial, &None)?;
        scope.execute(hal, &plan, &mut false)?;
        *scope.initial = None;
        Ok(())
    }
    pub(crate) fn hotkey_identity(&self, hal: &Hal) -> Result<String> {
        let profile = self.read(hal)?;
        if let Some(reason) = profile.blocked_reason {
            bail!("{reason}");
        }
        profile.current.identity()
    }
    pub(crate) fn toggle(&self, hal: &Hal, identity: &str) -> Result<bool> {
        let mut scope = self.scope()?;
        let _gate = hal.begin_write()?;
        let current = scope.read(hal)?.current;
        if current.identity()? != identity {
            bail!("触摸板设备已变化，请重新确认热键");
        }
        let enabled = !current.available()?;
        let plan = scope.prepare(
            hal,
            &Target {
                devices: current.devices.clone(),
                enabled,
                saved_enabled: enabled,
            },
        )?;
        if plan.before != current {
            bail!("触摸板在准备期间变化，未执行旧状态热键");
        }
        scope.execute(hal, &plan, &mut false)?;
        Ok(enabled)
    }
}
impl Scope<'_> {
    pub(crate) fn read(&self, hal: &Hal) -> Result<Profile> {
        self.controller.with_io(hal, |io| {
            let current = transaction::capture(io)?;
            let owner = io.ensure_available().err().map(|e| format!("{e:#}"));
            let changed = self
                .initial
                .as_ref()
                .and_then(|old| current.same_devices(old).err())
                .map(|e| format!("{e:#}"));
            let blocked_reason = current
                .available()
                .err()
                .map(|e| format!("{e:#}"))
                .or(owner.clone())
                .or(changed.clone());
            Ok(Profile {
                revision: current.revision(&self.initial)?,
                current,
                initial: self.initial.clone(),
                can_restore: self.initial.is_some() && changed.is_none() && owner.is_none(),
                restore_blocked_reason: changed.or(owner),
                blocked_reason,
            })
        })
    }
    pub(crate) fn capture(&self, hal: &Hal) -> Result<Option<Target>> {
        self.controller.with_io(hal, |io| {
            let current = transaction::capture(io)?;
            if current.devices.is_empty() {
                return Ok(None);
            }
            let enabled = current.available()?;
            if current.saved > 1 {
                bail!("原厂触摸板偏好未知，无法捕获可执行档案");
            }
            Ok(Some(Target {
                devices: current.devices,
                enabled,
                saved_enabled: current.saved != 0,
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
            transaction::prepare(before, after, &self.initial)
        })
    }
    pub(crate) fn execute(&mut self, hal: &Hal, plan: &Plan, needs: &mut bool) -> Result<()> {
        self.controller.with_io(hal, |io| {
            transaction::execute(io, &mut self.initial, plan, needs)
        })
    }
    pub(crate) fn rollback(&self, hal: &Hal, plan: &Plan) -> Result<()> {
        self.controller
            .with_io(hal, |io| transaction::rollback(io, plan))
    }
}
