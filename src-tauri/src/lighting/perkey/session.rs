use super::scope::{Change, Plan, Scope};
use super::*;

impl Scope<'_> {
    pub(crate) fn prepare_candidate(&mut self, hal: &Hal, plan: &Plan) -> Result<()> {
        if self.candidate.is_some() {
            anyhow::bail!("逐键候选会话重复准备");
        }
        let session =
            self.controller
                .prepare_session(hal, &plan.target, plan.before.identity.clone())?;
        self.candidate = Some(Change {
            session,
            target: plan.target.clone(),
            binding: plan.before.binding.clone(),
            started: false,
            was_running: false,
            restored: true,
            prior_error: None,
        });
        Ok(())
    }
    pub(crate) fn execute(
        &mut self,
        hal: &Hal,
        plan: &Plan,
        needs_rollback: &mut bool,
    ) -> Result<()> {
        let controller = self.controller;
        controller.with_io(hal, |raw, io| {
            let before = read(io)?;
            if raw != plan.before.raw || revision(io, &before) != plan.before.revision {
                anyhow::bail!("逐键状态或设备已变化，请重新预览");
            }
            let candidate = self
                .candidate
                .as_mut()
                .ok_or_else(|| anyhow!("逐键会话尚未准备"))?;
            if candidate.binding != binding(io, raw) || candidate.target != plan.target {
                anyhow::bail!("逐键候选设备或目标已变化");
            }
            if let Some(session) = &candidate.session {
                session.ready()?;
            }
            candidate.was_running = self
                .state
                .session
                .as_ref()
                .is_some_and(|session| session.running());
            candidate.prior_error = self.state.error.clone();
            if let Some(session) = &self.state.session {
                session.pause();
            }
            candidate.started = true;
            let result = write_target(
                io,
                &mut self.state.baseline,
                raw,
                &before,
                &plan.after,
                plan.replay,
                needs_rollback,
            );
            candidate.restored = !*needs_rollback;
            result
        })
    }
    pub(crate) fn arm(&self) -> Result<()> {
        if let Some(candidate) = &self.candidate {
            if candidate.started {
                if let Some(session) = &candidate.session {
                    session.activate()?;
                }
            }
        }
        Ok(())
    }
    pub(crate) fn rollback(&mut self, hal: &Hal, plan: &Plan) -> Result<()> {
        let result = self.controller.with_io(hal, |raw, io| {
            if binding(io, raw) != plan.before.binding || io.device() != plan.before.device {
                anyhow::bail!("逐键设备已变化，未恢复到其他键盘；首次快照已保留");
            }
            restore(io, &plan.before.snapshot)
        });
        if let Some(candidate) = &mut self.candidate {
            if let Some(session) = &candidate.session {
                session.pause();
            }
            candidate.restored = result.is_ok();
        }
        result
    }
    pub(crate) fn finish(&mut self, success: bool) -> Result<bool> {
        let Some(mut candidate) = self.candidate.take() else {
            return Ok(false);
        };
        if !candidate.started {
            return Ok(false);
        }
        if success {
            let previous = self.state.session.take();
            self.state.session = candidate.session.take();
            self.state.error = None;
            drop(previous);
            return Ok(false);
        }
        let restored = if candidate.restored && candidate.was_running {
            self.state
                .session
                .as_ref()
                .ok_or_else(|| anyhow!("原逐键事件会话已丢失"))?
                .activate()
        } else if candidate.was_running {
            Err(anyhow!("逐键前态尚未恢复，原按键响应保持暂停；可重试恢复"))
        } else {
            Ok(())
        };
        self.state.error = match &restored {
            Ok(()) => candidate.prior_error.take(),
            Err(error) => Some(format!("逐键按键响应恢复失败：{error:#}")),
        };
        restored.map(|_| candidate.was_running)
    }
    pub(super) fn run_single(&mut self, hal: &Hal, plan: &Plan) -> Result<()> {
        self.prepare_candidate(hal, plan)?;
        let mut touched = false;
        let result = (|| {
            let _gate = hal.begin_write()?;
            let result = self
                .execute(hal, plan, &mut touched)
                .and_then(|_| self.arm());
            if result.is_err() && touched {
                let recovery = self.rollback(hal, plan).map_err(|e| format!("{e:#}"));
                return Err(anyhow!(ccc_core::msr::with_recovery(
                    format!("{:#}", result.unwrap_err()),
                    recovery
                )));
            }
            result
        })();
        let finish = self.finish(result.is_ok()).map(|_| ());
        match result {
            Ok(()) => finish,
            Err(error) => Err(anyhow!(ccc_core::msr::with_recovery(
                format!("{error:#}"),
                finish.map_err(|e| format!("{e:#}"))
            ))),
        }
    }
}
