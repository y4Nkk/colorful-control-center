use super::controller::State;
use super::*;
use std::sync::MutexGuard;

pub(crate) struct Scope<'a> {
    pub(super) controller: &'a Controller,
    pub(super) state: MutexGuard<'a, State>,
    pub(super) candidate: Option<Change>,
}
pub(super) struct Observed {
    pub raw: u8,
    pub snapshot: Snapshot,
    pub values: Values,
    pub identity: String,
    pub device: DeviceInfo,
    pub binding: String,
    pub revision: String,
}
pub(crate) struct Plan {
    pub(super) before: Observed,
    pub(super) after: Snapshot,
    pub(super) target: Target,
    pub(super) replay: bool,
}
pub(super) struct Change {
    pub session: Option<runtime::Session>,
    pub target: Target,
    pub binding: String,
    pub started: bool,
    pub was_running: bool,
    pub restored: bool,
    pub prior_error: Option<String>,
}
impl Controller {
    pub(crate) fn scope(&self) -> Result<Scope<'_>> {
        Ok(Scope {
            controller: self,
            state: self.state.lock().map_err(|_| anyhow!("逐键控制状态异常"))?,
            candidate: None,
        })
    }
}
impl Scope<'_> {
    fn observe(&mut self, hal: &Hal) -> Result<Observed> {
        let result = self.controller.with_io(hal, |raw, io| {
            let snapshot = read(io)?;
            Ok(Observed {
                raw,
                values: effects::values(&snapshot)?,
                identity: io.identity(),
                device: io.device(),
                binding: binding(io, raw),
                revision: revision(io, &snapshot),
                snapshot,
            })
        });
        let current = match result {
            Ok(current) => current,
            Err(error) => {
                if let Some(session) = &self.state.session {
                    session.pause();
                }
                self.state.error = Some(format!("逐键读取失败，按键响应已暂停：{error:#}"));
                return Err(error);
            }
        };
        if let Some(session) = &self.state.session {
            if !session.matches(&current.values, &current.identity, &current.device) {
                session.pause();
                self.state.error =
                    Some("逐键保存值或设备已变化，按键响应已暂停；请重新应用".into());
            }
        }
        Ok(current)
    }
    pub(crate) fn capture(&mut self, hal: &Hal) -> Result<Option<Target>> {
        if detected(hal)?.is_none() {
            if let Some(session) = &self.state.session {
                session.pause();
            }
            return Ok(None);
        }
        let current = self.observe(hal)?;
        Ok(Some(Target {
            keyboard_type: current.raw,
            device: current.device,
            values: current.values,
        }))
    }
    pub(super) fn profile(&mut self, hal: &Hal) -> Result<Profile> {
        let current = self.observe(hal)?;
        Ok(Profile {
            revision: current.revision,
            presets: effects::presets(&current.snapshot, &current.device)?,
            current: current.values,
            device: current.device,
            can_restore: self.state.baseline.is_some(),
            reactive_running: self
                .state
                .session
                .as_ref()
                .is_some_and(|session| session.running()),
            reactive_error: self.state.error.clone().or_else(|| {
                self.state
                    .session
                    .as_ref()
                    .and_then(|session| session.error())
            }),
        })
    }
    pub(super) fn identity(&self, hal: &Hal) -> Result<String> {
        self.controller.with_io(hal, |raw, io| Ok(binding(io, raw)))
    }
    pub(super) fn hotkey_request(
        &mut self,
        hal: &Hal,
        expected: &str,
        delta: i8,
    ) -> Result<Request> {
        let mut current = self.observe(hal)?;
        if current.binding != expected {
            anyhow::bail!("逐键设备已变化，请重新确认热键");
        }
        current.values.brightness = super::super::hotkey::step(current.values.brightness, delta, 4);
        current.values.enabled = true;
        Ok(Request {
            expected_revision: current.revision,
            values: current.values,
        })
    }
    pub(super) fn hotkey_toggle_request(&mut self, hal: &Hal, expected: &str) -> Result<Request> {
        let mut current = self.observe(hal)?;
        if current.binding != expected {
            anyhow::bail!("逐键设备已变化，请重新确认热键");
        }
        current.values.enabled = !current.values.enabled;
        Ok(Request {
            expected_revision: current.revision,
            values: current.values,
        })
    }
    fn plan(&self, before: Observed, target: &Target, replay: bool) -> Result<Plan> {
        target.validate()?;
        if before.raw != target.keyboard_type || before.device != target.device {
            anyhow::bail!("逐键档案的型号、MCU能力或布局与当前设备不同");
        }
        if self
            .state
            .baseline
            .as_ref()
            .is_some_and(|(key, _)| key != &before.binding)
        {
            anyhow::bail!("逐键设备与首次快照不一致，恢复记录已保留");
        }
        if let Some(candidate) = &self.candidate {
            if candidate.binding != before.binding || candidate.target != *target {
                anyhow::bail!("逐键候选设备或目标已变化，未写入档案");
            }
            if let Some(session) = &candidate.session {
                session.ready()?;
            }
        }
        let after = model::target(&before.snapshot, &target.values, &before.device)?;
        Ok(Plan {
            before,
            after,
            target: target.clone(),
            replay,
        })
    }
    pub(crate) fn prepare(&mut self, hal: &Hal, target: &Target) -> Result<Plan> {
        let before = self.observe(hal)?;
        self.plan(
            before,
            target,
            target.values.enabled && target.values.pattern.reactive(),
        )
    }
    pub(super) fn reviewed(&mut self, hal: &Hal, request: &Request) -> Result<Plan> {
        let before = self.observe(hal)?;
        if before.revision != request.expected_revision {
            anyhow::bail!("逐键保存值或设备已变化，请重新读取并确认");
        }
        let target = Target {
            keyboard_type: before.raw,
            device: before.device.clone(),
            values: request.values.clone(),
        };
        self.plan(
            before,
            &target,
            target.values.enabled && target.values.pattern.reactive(),
        )
    }
    pub(super) fn initial(&mut self, hal: &Hal) -> Result<Plan> {
        let (key, saved) = self
            .state
            .baseline
            .clone()
            .ok_or_else(|| anyhow!("没有逐键初始快照"))?;
        let before = self.observe(hal)?;
        if before.binding != key {
            anyhow::bail!("逐键设备已变化，拒绝向其他设备恢复");
        }
        let target = Target {
            keyboard_type: before.raw,
            device: before.device.clone(),
            values: effects::values(&saved)?,
        };
        let mut plan = self.plan(before, &target, true)?;
        plan.after = saved;
        plan.after.settings[4..9].copy_from_slice(&plan.before.snapshot.settings[4..9]);
        plan.after.settings[49..76].copy_from_slice(&plan.before.snapshot.settings[49..76]);
        Ok(plan)
    }
}
