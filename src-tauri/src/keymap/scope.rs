use super::*;
use std::sync::MutexGuard;
use store::Store;
use transaction::{Pending, Plan};

pub(crate) struct Scope<'a> {
    pub(super) controller: &'a Controller,
    _owned: MutexGuard<'a, ()>,
    pub(super) change: Option<Change>,
}
pub(super) struct Change {
    pub record: model::Record,
    pub target: model::Target,
    pub revision: String,
    pub candidate: Option<launch::Session>,
    pub pending: Option<Pending>,
    pub started: bool,
    pub was_enabled: bool,
    pub restored: bool,
}
impl Controller {
    pub(crate) fn scope(&self) -> Result<Scope<'_>> {
        Ok(Scope {
            controller: self,
            _owned: self
                .mutation
                .lock()
                .map_err(|_| anyhow!("宏配置状态异常"))?,
            change: None,
        })
    }
    #[cfg(test)]
    pub(crate) fn with_test_io(mut self, io: Arc<dyn Io + Send + Sync>) -> Self {
        self.test_io = Some(io);
        self
    }
}
impl Scope<'_> {
    pub(super) fn with_io<T>(
        &self,
        hal: &Hal,
        action: impl FnOnce(&dyn Io) -> Result<T>,
    ) -> Result<T> {
        #[cfg(test)]
        {
            let _ = hal;
            return match &self.controller.test_io {
                Some(io) => action(io.as_ref()),
                None => bail!("隔离测试未注入映射IO，未读取原厂配置或调用过滤库/事件日志"),
            };
        }
        #[cfg(not(test))]
        action(&native::Native {
            hal,
            controller: self.controller,
        })
    }
    pub(super) fn read(&self, hal: &Hal) -> Result<Option<Profile>> {
        self.with_io(hal, |io| {
            let mut profile = read(io, &store::FileStore(self.controller.path.clone()))?;
            if let Some(profile) = profile.as_mut() {
                profile.programs = self.controller.launchers.status()?;
            }
            Ok(profile)
        })
    }
    pub(crate) fn capture(&self, hal: &Hal) -> Result<Option<model::Target>> {
        let capability: [u8; 15] = hal
            .with_dchu(|io| io.read_bios_capabilities(0, 15))?
            .try_into()
            .map_err(|_| anyhow!("宏能力页长度错误"))?;
        if !supported(&capability)? {
            if store::FileStore(self.controller.path.clone())
                .load()?
                .record
                .is_some()
            {
                bail!("映射能力已丢失，恢复记录已保留");
            }
            return Ok(None);
        }
        self.with_io(hal, |io| {
            let frame = capture(io, &store::FileStore(self.controller.path.clone()))?;
            frame.review(&frame.revision()?, false)?;
            let current = frame.current().context("原厂活动映射不可读，无法捕获")?;
            let mut configuration = if let Some(record) = &frame.loaded.record {
                model::subset(current, &record.owned)?
            } else {
                current.clone()
            };
            for device in &mut configuration.devices {
                device.enabled = frame.facts.flags[model::index(device.device)] == 1;
            }
            Ok(Some(model::Target {
                configuration,
                programs_enabled: self.controller.launchers.enabled()?,
            }))
        })
    }
    pub(crate) fn prepare(&self, hal: &Hal, target: &model::Target) -> Result<Plan> {
        self.check_recording()?;
        self.with_io(hal, |io| {
            let frame = capture(io, &store::FileStore(self.controller.path.clone()))?;
            let plan = transaction::prepare(frame, target)?;
            target.require_program_scope(&plan.record)?;
            self.require_candidate(&plan)?;
            Ok(plan)
        })
    }
    pub(super) fn reviewed(&self, hal: &Hal, request: &Request) -> Result<Plan> {
        self.check_recording()?;
        self.with_io(hal, |io| {
            let frame = capture(io, &store::FileStore(self.controller.path.clone()))?;
            frame.review(&request.expected_revision, false)?;
            transaction::prepare(
                frame,
                &model::Target {
                    configuration: request.configuration.clone(),
                    programs_enabled: true,
                },
            )
        })
    }
    pub(super) fn initial(&self, hal: &Hal, expected: &str) -> Result<Plan> {
        self.check_recording()?;
        self.with_io(hal, |io| {
            let frame = capture(io, &store::FileStore(self.controller.path.clone()))?;
            frame.review(expected, true)?;
            transaction::initial(frame)
        })
    }
    pub(super) fn check_recording(&self) -> Result<()> {
        if self.controller.recording.active()? {
            bail!("请先结束宏录制，再应用或恢复映射");
        }
        Ok(())
    }
    pub(super) fn require_candidate(&self, plan: &Plan) -> Result<()> {
        if let Some(change) = &self.change {
            if change.revision != plan.revision
                || change.record != plan.record
                || change.target != plan.target
            {
                bail!("映射来源、恢复记录或目标已变化，未写入档案");
            }
            if let Some(session) = &change.candidate {
                session.ready()?;
                if !session.matches(&plan.record) {
                    bail!("程序监听候选目标不一致");
                }
            }
        }
        Ok(())
    }
}
