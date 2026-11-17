use super::*;
use scope::{Change, Scope};

impl Scope<'_> {
    pub(crate) fn prepare_candidate(&mut self, hal: &Hal, plan: &Plan) -> Result<()> {
        if self.change.is_some() {
            bail!("映射候选重复准备");
        }
        let candidate = if plan.target.programs_enabled {
            self.with_io(hal, |io| {
                io.prepare_programs(&plan.record, hal.control_gate())
            })?
        } else {
            None
        };
        if plan.target.programs_enabled && launch::required(&plan.record) && candidate.is_none() {
            bail!("程序事件工厂没有准备所需监听，未执行映射");
        }
        self.change = Some(Change {
            record: plan.record.clone(),
            target: plan.target.clone(),
            revision: plan.revision.clone(),
            candidate,
            pending: None,
            started: false,
            was_enabled: false,
            restored: true,
        });
        Ok(())
    }
    pub(crate) fn execute(
        &mut self,
        hal: &Hal,
        plan: &Plan,
        needs_rollback: &mut bool,
    ) -> Result<()> {
        self.check_recording()?;
        self.require_candidate(plan)?;
        self.with_io(hal, |io| {
            if capture(io, &store::FileStore(self.controller.path.clone()))?.revision()?
                != plan.revision
            {
                bail!("映射来源或状态已变化，请重新预览");
            }
            Ok(())
        })?;
        let mut change = self.change.take().context("映射候选尚未准备")?;
        let result = (|| {
            change.was_enabled = self.controller.launchers.pause()?;
            change.started = true;
            self.with_io(hal, |io| {
                let store = store::FileStore(self.controller.path.clone());
                let result = transaction::execute(io, &store, plan, &mut change.pending);
                if let Err(error) = result {
                    if let Some(pending) = &change.pending {
                        let recovery = transaction::rollback(io, &store, plan, pending);
                        if recovery.is_ok() {
                            change.pending = None;
                        }
                        bail!(ccc_core::msr::with_recovery(
                            format!("{error:#}"),
                            recovery.map_err(|e| format!("{e:#}"))
                        ));
                    }
                    return Err(error);
                }
                Ok(())
            })
        })();
        *needs_rollback = change.pending.is_some();
        change.restored = !*needs_rollback;
        self.change = Some(change);
        result
    }
    pub(crate) fn commit(&mut self, hal: &Hal, plan: &Plan) -> Result<()> {
        self.require_candidate(plan)?;
        let mut change = self.change.take().context("映射候选尚未准备")?;
        let result = self.with_io(hal, |io| {
            let pending = change.pending.as_mut().context("映射尚未执行")?;
            transaction::commit(
                io,
                &store::FileStore(self.controller.path.clone()),
                plan,
                pending,
            )?;
            if let Some(session) = &change.candidate {
                session.activate()?;
            }
            Ok(())
        });
        self.change = Some(change);
        result
    }
    pub(crate) fn rollback(&mut self, hal: &Hal, plan: &Plan) -> Result<()> {
        let mut change = self.change.take().context("映射候选尚未准备")?;
        if let Some(session) = &change.candidate {
            session.pause();
        }
        let result = self.with_io(hal, |io| {
            if let Some(pending) = &change.pending {
                transaction::rollback(
                    io,
                    &store::FileStore(self.controller.path.clone()),
                    plan,
                    pending,
                )?;
            }
            Ok(())
        });
        change.restored = result.is_ok();
        if result.is_ok() {
            change.pending = None;
        }
        self.change = Some(change);
        result
    }
    pub(crate) fn finish(&mut self, success: bool) -> Result<bool> {
        let Some(mut change) = self.change.take() else {
            return Ok(false);
        };
        if !change.started {
            return Ok(false);
        }
        if success {
            self.controller.launchers.replace(change.candidate.take())?;
            return Ok(false);
        }
        if change.was_enabled {
            if !change.restored {
                bail!("映射前态尚未恢复，原程序监听保持暂停；请恢复初始配置");
            }
            self.controller.launchers.resume()?;
            return Ok(true);
        }
        Ok(false)
    }
    pub(super) fn run_single(&mut self, hal: &Hal, plan: &Plan) -> Result<()> {
        self.prepare_candidate(hal, plan)?;
        let mut touched = false;
        let result = (|| {
            let _gate = hal.begin_write()?;
            let result = self
                .execute(hal, plan, &mut touched)
                .and_then(|_| self.commit(hal, plan));
            if result.is_err() && touched {
                let recovery = self.rollback(hal, plan).map_err(|e| format!("{e:#}"));
                bail!(ccc_core::msr::with_recovery(
                    format!("{:#}", result.err().unwrap()),
                    recovery
                ));
            }
            result
        })();
        let finish = self.finish(result.is_ok());
        match result {
            Ok(()) => finish.map(|_| ()),
            Err(error) => Err(anyhow!(ccc_core::msr::with_recovery(
                format!("{error:#}"),
                finish.map(|_| ()).map_err(|e| format!("{e:#}"))
            ))),
        }
    }
    pub(super) fn retry_programs(&self, hal: &Hal, expected: &str) -> Result<()> {
        self.check_recording()?;
        let record = {
            let _gate = hal.begin_write()?;
            self.with_io(hal, |io| {
                let frame = capture(io, &store::FileStore(self.controller.path.clone()))?;
                frame.review(expected, false)?;
                frame.loaded.record.context("没有本应用已确认的程序映射")
            })?
        };
        let candidate = self.with_io(hal, |io| io.prepare_programs(&record, hal.control_gate()))?;
        if launch::required(&record) && candidate.is_none() {
            bail!("程序监听未准备，未替换原会话");
        }
        {
            let _gate = hal.begin_write()?;
            self.with_io(hal, |io| {
                capture(io, &store::FileStore(self.controller.path.clone()))?
                    .review(expected, false)
            })?;
            if let Some(session) = &candidate {
                session.ready()?;
            }
            let was_enabled = self.controller.launchers.pause()?;
            if let Some(session) = &candidate {
                if let Err(error) = session.activate() {
                    let restored = if was_enabled {
                        self.controller.launchers.resume()
                    } else {
                        Ok(())
                    };
                    bail!(ccc_core::msr::with_recovery(
                        format!("{error:#}"),
                        restored.map_err(|e| format!("{e:#}"))
                    ));
                }
            }
        }
        self.controller.launchers.replace(candidate)
    }
}
