//! 显示域唯一状态：独立切换与档案共用试用、首次基线及恢复监护。
use anyhow::{Result, anyhow, bail};
use serde::Serialize;
use std::{collections::BTreeMap, sync::Mutex};
#[path = "display/automatic/mod.rs"]
pub(crate) mod automatic;
#[path = "display/guardian/mod.rs"]
pub mod guardian;
#[path = "display/model.rs"]
mod model;
#[path = "display/profile.rs"]
pub mod profile;
#[path = "display/trial.rs"]
mod trial;
#[path = "display/worker.rs"]
mod worker;
pub use model::{Access, Display, Mode};
pub use worker::run_child;
#[path = "hal/display.rs"]
pub mod native;
#[cfg(test)]
#[path = "display/test_fixture.rs"]
pub(crate) mod test_fixture;
#[cfg(test)]
#[path = "display/tests.rs"]
mod tests;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pending {
    pub before: Vec<Mode>,
    pub targets: Vec<Mode>,
    pub rotation: bool,
    pub automatic: bool,
    pub trial_id: Option<u64>,
    pub remaining_ms: Option<u64>,
    pub profile_session: Option<u64>,
    pub error: Option<String>,
}
struct Trial {
    id: u64,
    plan: guardian::Plan,
    deadline: u64,
    guard: Box<dyn guardian::Monitor>,
    owner: Owner,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Owner {
    Manual,
    Automatic,
    Profile(u64),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AutomaticResult {
    Kept,
    Restored,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AutomaticOutcome {
    id: u64,
    result: AutomaticResult,
}
struct Retained {
    id: u64,
    changes: Vec<guardian::Change>,
}
#[derive(Default)]
struct State {
    sequence: u64,
    baseline: BTreeMap<String, Mode>,
    trial: Option<Trial>,
    retained: Option<Retained>,
    automatic_outcome: Option<AutomaticOutcome>,
    error: Option<String>,
}
impl State {
    fn pending(&self, now: u64) -> Option<Pending> {
        self.trial
            .as_ref()
            .map(|trial| Pending {
                before: trial.plan.before(),
                targets: trial.plan.targets(),
                rotation: trial.plan.changes.iter().any(|change| change.is_rotation()),
                automatic: trial.owner == Owner::Automatic,
                trial_id: Some(trial.id),
                remaining_ms: Some(trial.deadline.saturating_sub(now)),
                profile_session: match trial.owner {
                    Owner::Profile(id) => Some(id),
                    _ => None,
                },
                error: self.error.clone(),
            })
            .or_else(|| {
                self.retained.as_ref().map(|retained| Pending {
                    before: retained
                        .changes
                        .iter()
                        .map(|change| change.before.clone())
                        .collect(),
                    targets: retained
                        .changes
                        .iter()
                        .map(|change| change.target.clone())
                        .collect(),
                    rotation: false,
                    automatic: false,
                    trial_id: None,
                    remaining_ms: None,
                    profile_session: Some(retained.id),
                    error: self.error.clone(),
                })
            })
    }
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub displays: Vec<Display>,
    pub rotation: Option<guardian::Change>,
    pub pending: Option<Pending>,
    pub can_restore: bool,
    pub error: Option<String>,
    pub blocked_reason: Option<String>,
}
pub struct Controller {
    io: Box<dyn Access>,
    state: Mutex<State>,
    guards: Box<dyn guardian::Factory>,
}
impl Controller {
    pub fn trial_pending(&self) -> Result<bool> {
        let state = self.lock()?;
        Ok(state.trial.is_some() || state.retained.is_some() || state.automatic_outcome.is_some())
    }
    pub fn apply_runtime(
        &self,
        hal: &crate::hal::Hal,
        expected: &Mode,
        frequency: u32,
    ) -> Result<()> {
        let _write = hal.begin_write()?;
        crate::hal::oem_owner::ensure_owner_absent()?;
        self.apply(expected, frequency, now_ms())
    }
    pub fn apply_rotation_runtime(&self, hal: &crate::hal::Hal, expected: &Mode) -> Result<()> {
        let _write = hal.begin_write()?;
        crate::hal::oem_owner::ensure_owner_absent()?;
        self.apply_rotation(expected, now_ms())
    }
    pub fn primary(&self) -> Result<Mode> {
        primary_mode(&self.io.list()?)
    }
    pub fn restore_runtime(&self, hal: &crate::hal::Hal) -> Result<()> {
        let _write = hal.begin_write()?;
        crate::hal::oem_owner::ensure_owner_absent()?;
        self.restore()
    }
    pub fn keep_runtime(&self, hal: &crate::hal::Hal, id: u64, expected: &Mode) -> Result<()> {
        let _write = hal.begin_write()?;
        self.keep(id, expected, now_ms())
    }
    pub fn cancel_runtime(&self, hal: &crate::hal::Hal, id: u64) -> Result<()> {
        let _write = hal.begin_write()?;
        self.cancel(id)
    }
    pub fn tick_runtime(&self, hal: &crate::hal::Hal) -> Result<()> {
        if self.lock()?.trial.is_none() {
            return Ok(());
        }
        let _write = hal.begin_write()?;
        self.tick(now_ms())
    }
    pub(crate) fn release_on_exit(&self, hal: &crate::hal::Hal) -> Result<()> {
        let _write = hal.begin_write()?;
        let mut state = self.lock()?;
        if let Some(trial) = state.trial.as_mut() {
            self.recover_trial(trial)?;
            state.trial = None;
        }
        if let Some(retained) = &state.retained {
            guardian::restore(self.io.as_ref(), &retained.changes)?;
            state.retained = None;
        }
        Ok(())
    }
    pub fn new(io: impl Access + 'static) -> Self {
        Self::with_guards(io, guardian::ProcessFactory)
    }
    pub fn with_guards(
        io: impl Access + 'static,
        guards: impl guardian::Factory + 'static,
    ) -> Self {
        Self {
            io: Box::new(io),
            state: Mutex::new(State::default()),
            guards: Box::new(guards),
        }
    }
    fn lock(&self) -> Result<std::sync::MutexGuard<'_, State>> {
        self.state.lock().map_err(|_| anyhow!("显示恢复状态异常"))
    }
    pub fn read(&self) -> Result<Profile> {
        let state = self.lock()?;
        let displays = self.io.list()?;
        let rotation = primary_mode(&displays).ok().and_then(|before| {
            before
                .oem_rotation()
                .ok()
                .map(|target| guardian::Change { before, target })
        });
        Ok(Profile {
            displays,
            rotation,
            pending: state.pending(now_ms()),
            can_restore: !state.baseline.is_empty(),
            error: state.error.clone(),
            blocked_reason: None,
        })
    }
    pub fn restore(&self) -> Result<()> {
        let mut state = self.lock()?;
        if state.retained.is_some()
            || state
                .trial
                .as_ref()
                .is_some_and(|trial| trial.owner != Owner::Manual)
            || state.automatic_outcome.is_some()
        {
            bail!("先结束自动或档案显示试用并处理恢复结果");
        }
        if state.baseline.is_empty() {
            bail!("没有初始显示模式快照");
        }
        let mut errors = Vec::new();
        if let Some(mut trial) = state.trial.take() {
            if let Err(error) = self.recover_trial(&mut trial) {
                errors.push(format!("解除本次显示试用：{error:#}"));
                state.trial = Some(trial);
            }
        }
        for mode in state.baseline.values() {
            if state.trial.as_ref().is_some_and(|trial| {
                trial
                    .plan
                    .changes
                    .iter()
                    .any(|change| change.before.device == mode.device)
            }) {
                continue;
            }
            if let Err(error) = self.io.set(mode) {
                errors.push(format!("{}：{error:#}", mode.name));
            }
        }
        if !errors.is_empty() {
            let message = errors.join("；");
            state.error = Some(message.clone());
            bail!(message);
        }
        state.error = None;
        Ok(())
    }
}
fn primary_mode(displays: &[Display]) -> Result<Mode> {
    let mut primary = displays.iter().filter(|display| display.primary);
    let display = primary.next().ok_or_else(|| anyhow!("没有活动主显示器"))?;
    if primary.next().is_some() {
        bail!("主显示器状态不明确");
    }
    display.current.validate()?;
    Ok(display.current.clone())
}
pub fn now_ms() -> u64 {
    static EPOCH: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    EPOCH
        .get_or_init(std::time::Instant::now)
        .elapsed()
        .as_millis() as u64
}
