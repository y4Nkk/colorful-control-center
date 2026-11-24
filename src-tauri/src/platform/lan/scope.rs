use super::*;
use authorization::Authorized;
use std::sync::MutexGuard;
use worker::{Intent, Operation};

pub(crate) struct Plan {
    operation: Operation,
    plans: Vec<transaction::Plan>,
}
struct Prepared {
    fingerprint: String,
    session: Box<dyn Authorized>,
}
pub(crate) struct Scope<'a> {
    owner: &'a Controller,
    _lock: MutexGuard<'a, ()>,
    prepared: Option<Prepared>,
    paused: Option<ccc_core::software_fan::Curves>,
    authorization_checked: bool,
}
impl Controller {
    pub(crate) fn scope(&self) -> Result<Scope<'_>> {
        Ok(Scope {
            owner: self,
            _lock: self
                .mutation
                .lock()
                .map_err(|_| anyhow!("LAN控制状态异常"))?,
            prepared: None,
            paused: None,
            authorization_checked: false,
        })
    }
}
impl Scope<'_> {
    fn environment(&self) -> Result<&dyn Environment> {
        self.owner.environment()
    }
    fn source(&self, hal: &Hal) -> Result<(Frame, Record, String)> {
        let environment = self.environment()?;
        let record = store::read(&self.owner.path)?;
        let frame = environment.frame(hal, !record.devices.is_empty())?;
        frame.validate()?;
        let revision = model::revision(&frame, &record)?;
        Ok((frame, record, revision))
    }
    fn available(&self, hal: &Hal, allow_software: bool) -> Result<()> {
        hal.ensure_mode_change_allowed()?;
        if !allow_software && crate::fanctl::software::active(hal)? {
            bail!("请先确认软件风扇已归还，再请求网络策略系统授权");
        }
        self.environment()?.ensure_owner_absent()
    }
    pub fn read(&self, hal: &Hal) -> Result<Option<Profile>> {
        let (frame, record, revision) = self.source(hal)?;
        if !frame.capabilities.any() && record.devices.is_empty() {
            return Ok(None);
        }
        Ok(Some(Profile {
            revision,
            frame,
            initial: record.devices,
            last_error: record.last_error,
            blocked_reason: self
                .available(hal, false)
                .err()
                .map(|error| format!("{error:#}")),
        }))
    }
    pub fn capture(&self, hal: &Hal) -> Result<Option<Target>> {
        Target::capture(&self.source(hal)?.0)
    }
    pub fn preview(&self, hal: &Hal, request: &Request) -> Result<Vec<Change>> {
        let (frame, record, revision) = self.source(hal)?;
        if revision != request.expected_revision {
            bail!("LAN预览、驱动或初始记录已变化");
        }
        let operation = Operation {
            intent: Intent::Apply {
                request: request.clone(),
            },
            recovery_file: self.owner.path.clone(),
        };
        Ok(operation
            .plans(&frame, &record)?
            .into_iter()
            .flat_map(|plan| plan.changes)
            .collect())
    }
    pub fn prepare_request(
        &self,
        hal: &Hal,
        request: &Request,
        restoring: bool,
    ) -> Result<Option<Plan>> {
        self.available(hal, false)?;
        let intent = if restoring {
            Intent::Restore {
                expected_revision: request.expected_revision.clone(),
                device_id: request.device_id.clone(),
            }
        } else {
            Intent::Apply {
                request: request.clone(),
            }
        };
        self.prepare(hal, intent)
    }
    pub fn prepare_profile(&self, hal: &Hal, target: &Target) -> Result<Option<Plan>> {
        self.available(hal, true)?;
        let (_, _, expected_revision) = self.source(hal)?;
        self.prepare(
            hal,
            Intent::Profile {
                expected_revision,
                target: target.clone(),
            },
        )
    }
    fn prepare(&self, hal: &Hal, intent: Intent) -> Result<Option<Plan>> {
        let (frame, record, _) = self.source(hal)?;
        let operation = Operation {
            intent,
            recovery_file: self.owner.path.clone(),
        };
        let plans = operation.plans(&frame, &record)?;
        if self.authorization_checked && self.prepared.is_none() && !plans.is_empty() {
            bail!("LAN预检后新增变更，需要重新预览并准备授权");
        }
        let fingerprint = model::digest(&operation)?;
        if self
            .prepared
            .as_ref()
            .is_some_and(|prepared| prepared.fingerprint != fingerprint)
        {
            bail!("LAN授权期间预览、驱动或恢复记录已变化，未发送执行许可");
        }
        Ok((!plans.is_empty()).then_some(Plan { operation, plans }))
    }
    pub fn authorize(&mut self, plan: &Plan) -> Result<()> {
        if self.prepared.is_some() {
            bail!("LAN授权会话已准备");
        }
        let session = self.environment()?.authorize(plan.operation.clone())?;
        self.prepared = Some(Prepared {
            fingerprint: model::digest(&plan.operation)?,
            session,
        });
        self.authorization_checked = true;
        Ok(())
    }
    pub fn authorize_profile(&mut self, plan: Option<&Plan>) -> Result<()> {
        if let Some(plan) = plan {
            self.authorize(plan)?;
        }
        self.authorization_checked = true;
        Ok(())
    }
    fn session(&mut self, plan: &Plan) -> Result<&mut Box<dyn Authorized>> {
        let prepared = self.prepared.as_mut().context("LAN档案尚未准备系统授权")?;
        if prepared.fingerprint != model::digest(&plan.operation)? {
            bail!("LAN授权绑定已变化");
        }
        Ok(&mut prepared.session)
    }
    fn pause(&mut self, hal: &Hal, allow_software: bool) -> Result<()> {
        if allow_software && crate::fanctl::software::active(hal)? {
            if self.paused.is_none() {
                self.paused = crate::fanctl::software::read(hal)?;
            }
            crate::fanctl::software::stop_safely(hal)?;
        }
        Ok(())
    }
    fn resume(&mut self, hal: &Hal) -> Result<()> {
        if let Some(curves) = &self.paused {
            crate::fanctl::software::apply(hal, curves)?;
            self.paused = None;
        }
        Ok(())
    }
    pub fn execute(
        &mut self,
        hal: &Hal,
        plan: &Plan,
        touched: &mut bool,
        allow_software: bool,
    ) -> Result<()> {
        let (frame, record, revision) = self.source(hal)?;
        if revision != plan.operation.revision() {
            bail!("LAN授权期间预览、驱动或恢复记录已变化，未发送执行许可");
        }
        plan.operation.plans(&frame, &record)?;
        self.available(hal, allow_software)?;
        self.session(plan)?;
        self.pause(hal, allow_software)?;
        let result = (|| -> Result<()> {
            let session = self.session(plan)?;
            let result = session.execute();
            *touched = session.dirty();
            result?;
            let (now, _, _) = self.source(hal)?;
            for target in &plan.plans {
                let device = now.device(&target.expected.identity.instance)?;
                if device.identity != target.expected.identity
                    || target
                        .changes
                        .iter()
                        .any(|change| device.values[&change.field] != change.target)
                {
                    bail!("LAN授权后回读不一致，请恢复初始字段");
                }
            }
            Ok(())
        })();
        combine(result, self.resume(hal), "恢复软件风扇")
    }
    pub fn rollback(
        &mut self,
        hal: &Hal,
        plan: &Plan,
        touched: &mut bool,
        allow_software: bool,
    ) -> Result<()> {
        if !*touched {
            return Ok(());
        }
        self.available(hal, allow_software)?;
        self.session(plan)?;
        self.pause(hal, allow_software)?;
        let result = (|| {
            let session = self.session(plan)?;
            let result = session.rollback();
            *touched = session.dirty();
            result
        })();
        combine(result, self.resume(hal), "恢复软件风扇")
    }
    pub fn finish(&mut self) -> Result<()> {
        self.prepared
            .take()
            .map(|prepared| prepared.session.finish())
            .unwrap_or(Ok(()))
    }
}
pub(super) fn combine(result: Result<()>, recovery: Result<()>, name: &str) -> Result<()> {
    match (result, recovery) {
        (Err(error), Err(recovery)) => bail!("{error:#}；{name}失败：{recovery:#}"),
        (Err(error), _) => Err(error),
        (_, result) => result,
    }
}
