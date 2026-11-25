//! 档案应用会话：显示试用先确认，其余目标重新预检后执行。
use super::{
    ApplyStep, Devices, ProfileFile,
    apply::review::{self, DisplayAction, Reviewed},
};
use crate::{display, gpuctl::GpuController, hal::Hal, powerctl};
use anyhow::{Result, anyhow, bail};
use serde::Serialize;
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use tauri::Manager;

mod app;
#[cfg(test)]
mod busy_tests;
mod report;
pub(super) use app::apply_automatic;
pub(crate) use app::apply_startup;
pub use app::{apply_preview, cancel, keep};
pub(crate) use report::{describe_application, log_application};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Origin {
    Manual,
    Startup,
    Ac,
    Battery,
    Resume,
}
impl Origin {
    pub fn automatic(self) -> bool {
        matches!(self, Self::Ac | Self::Battery | Self::Resume)
    }
}

#[derive(Clone)]
pub(crate) struct Permission {
    check: Arc<dyn Fn() -> Result<()> + Send + Sync>,
    system: Arc<dyn Fn() -> Result<()> + Send + Sync>,
    mode_driver:
        Option<Arc<dyn Fn(&powerctl::mode::profile::ApprovalPreview) -> Result<()> + Send + Sync>>,
}
impl Permission {
    #[cfg(test)]
    pub(super) fn new(check: impl Fn() -> Result<()> + Send + Sync + 'static) -> Self {
        Self {
            check: Arc::new(check),
            system: Arc::new(|| Ok(())),
            mode_driver: None,
        }
    }
    pub(crate) fn with_mode_driver(
        check: impl Fn() -> Result<()> + Send + Sync + 'static,
        bind: impl Fn(&powerctl::mode::profile::ApprovalPreview) -> Result<()> + Send + Sync + 'static,
        system: impl Fn() -> Result<()> + Send + Sync + 'static,
    ) -> Self {
        Self {
            check: Arc::new(check),
            system: Arc::new(system),
            mode_driver: Some(Arc::new(bind)),
        }
    }
    pub(crate) fn check(&self) -> Result<()> {
        (self.check)()
    }
    pub(crate) fn require_system(&self) -> Result<()> {
        self.check()?;
        (self.system)()
    }
    pub(crate) fn bind_mode_driver(
        &self,
        facts: &powerctl::mode::profile::ApprovalPreview,
    ) -> Result<()> {
        self.check()?;
        self.mode_driver
            .as_ref()
            .ok_or_else(|| anyhow!("后台档案没有GPU驱动绑定许可"))?(facts)
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Application {
    pub id: u64,
    pub revision: u64,
    pub origin: Origin,
    pub targets_applied: bool,
    pub name: String,
    pub steps: Vec<ApplyStep>,
    pub pending: Option<display::Pending>,
}
impl Application {
    fn advance(&mut self) -> Result<()> {
        self.revision = self
            .revision
            .checked_add(1)
            .filter(|revision| *revision <= 9_007_199_254_740_991)
            .ok_or_else(|| anyhow!("档案应用报告修订已耗尽"))?;
        Ok(())
    }
}
#[derive(Clone)]
struct Pending {
    permission: Option<Permission>,
    id: u64,
    dir: PathBuf,
    file: ProfileFile,
    mode: Option<powerctl::mode::Target>,
    gpu_power: Option<crate::gpu_power::profile::Review>,
    graphics_mode: Option<crate::graphics_mode::profile::Review>,
}
#[derive(Default)]
struct State {
    next_id: u64,
    pending: Option<Pending>,
    report: Option<Application>,
    last_startup: Option<Application>,
}
impl State {
    fn start(&mut self, runtime: &Runtime<'_>, reviewed: Reviewed<'_>) -> Result<u64> {
        self.next_id = self
            .next_id
            .checked_add(1)
            .filter(|id| *id <= 9_007_199_254_740_991)
            .ok_or_else(|| anyhow!("档案应用会话标识已耗尽"))?;
        let id = self.next_id;
        if let Some(previous) = self.report.take()
            && previous.origin == Origin::Startup
        {
            self.last_startup = Some(previous);
        }
        self.report = Some(Application {
            id,
            revision: 1,
            origin: runtime.origin,
            targets_applied: false,
            name: reviewed.file.name.clone(),
            steps: vec![],
            pending: None,
        });
        self.pending = Some(Pending {
            permission: runtime.permission.clone(),
            id,
            dir: runtime.dir.into(),
            file: reviewed.file.clone(),
            mode: reviewed.mode.cloned(),
            gpu_power: reviewed.gpu_power.cloned(),
            graphics_mode: reviewed.graphics_mode.cloned(),
        });
        Ok(id)
    }
}
#[derive(Default)]
pub struct Controller(Mutex<State>);

pub(super) struct Runtime<'a> {
    pub permission: Option<Permission>,
    pub origin: Origin,
    pub hal: &'a Hal,
    pub gpu: &'a GpuController,
    pub devices: Devices<'a>,
    pub dir: &'a Path,
}
fn step(name: &str, result: Result<()>) -> ApplyStep {
    ApplyStep {
        step: name.into(),
        ok: result.is_ok(),
        message: result
            .err()
            .map_or_else(|| "已完成".into(), |error| format!("{error:#}")),
    }
}
impl Controller {
    pub(crate) fn is_busy(&self, display: &display::Controller) -> Result<bool> {
        let mut state = match self.0.try_lock() {
            Ok(state) => state,
            Err(std::sync::TryLockError::WouldBlock) => return Ok(true),
            Err(std::sync::TryLockError::Poisoned(_)) => bail!("档案应用状态异常"),
        };
        Self::refresh(&mut state, display)?;
        Ok(state.pending.is_some())
    }
    fn lock(&self) -> Result<std::sync::MutexGuard<'_, State>> {
        self.0.lock().map_err(|_| anyhow!("档案应用状态异常"))
    }
    fn refresh(state: &mut State, display: &display::Controller) -> Result<()> {
        let Some(pending) = &state.pending else {
            return Ok(());
        };
        let current = display.profile_pending(pending.id, display::now_ms())?;
        if let Some(report) = &mut state.report {
            let previous_error = report
                .pending
                .as_ref()
                .and_then(|pending| pending.error.as_ref());
            let current_error = current.as_ref().and_then(|pending| pending.error.as_ref());
            let issue_changed = previous_error != current_error;
            report.pending = current;
            if report.pending.is_none() {
                report.advance()?;
                report.steps.push(step(
                    "显示试用结束",
                    Err(anyhow!("试用已恢复，未执行其余档案设置")),
                ));
                state.pending = None;
                crate::profiles::application::log_application(report);
            } else if issue_changed {
                report.advance()?;
                crate::profiles::application::log_application(report);
            }
        }
        Ok(())
    }
    pub fn read(&self, display: &display::Controller) -> Result<Option<Application>> {
        let mut state = self.lock()?;
        Self::refresh(&mut state, display)?;
        Ok(state.report.clone())
    }
    pub fn startup_report(&self, display: &display::Controller) -> Result<Option<Application>> {
        let mut state = self.lock()?;
        Self::refresh(&mut state, display)?;
        let report = state
            .report
            .as_ref()
            .filter(|report| report.origin == Origin::Startup)
            .or(state.last_startup.as_ref());
        Ok(report.cloned())
    }
    pub(super) fn begin(
        &self,
        runtime: Runtime<'_>,
        file: &ProfileFile,
        mode: Option<&powerctl::mode::Target>,
        display: Option<&display::profile::Review>,
        gpu_power: Option<&crate::gpu_power::profile::Review>,
        graphics_mode: Option<&crate::graphics_mode::profile::Review>,
    ) -> Result<Application> {
        let mut state = self.lock()?;
        anyhow::ensure!(
            (runtime.origin != Origin::Manual) == runtime.permission.is_some(),
            "后台档案来源必须绑定有效授权"
        );
        if let Some(permission) = &runtime.permission {
            permission.check()?;
        }
        Self::refresh(&mut state, runtime.devices.display)?;
        if state.pending.is_some() {
            bail!("先结束当前档案显示确认或恢复");
        }
        let reviewed = Reviewed {
            permission: runtime.permission.as_ref(),
            dir: runtime.dir,
            file,
            mode,
            display,
            gpu_power,
            graphics_mode,
        };
        let id = state.start(&runtime, reviewed)?;
        let started = review::display_action(
            runtime.hal,
            runtime.gpu,
            runtime.devices,
            reviewed,
            DisplayAction::Begin(id),
        );
        match started {
            Ok(true) => {
                state.report.as_mut().unwrap().pending = runtime
                    .devices
                    .display
                    .profile_pending(id, display::now_ms())?;
            }
            Ok(false) => {
                self.remainder(&mut state, &runtime, reviewed, false)?;
            }
            Err(error) => {
                state
                    .report
                    .as_mut()
                    .unwrap()
                    .steps
                    .push(step("档案预检与显示试用", Err(error)));
                self.update_pending(&mut state, runtime.devices.display)?;
            }
        }
        Ok(state.report.clone().expect("已登记应用结果"))
    }
    pub(super) fn keep(&self, runtime: Runtime<'_>, id: u64) -> Result<Application> {
        let mut state = self.lock()?;
        Self::refresh(&mut state, runtime.devices.display)?;
        let pending = state
            .pending
            .as_ref()
            .filter(|pending| pending.id == id)
            .ok_or_else(|| anyhow!("档案显示试用已结束或不属于此请求"))?
            .clone();
        if state.report.as_ref().is_some_and(|report| {
            report.steps.iter().any(|step| !step.ok)
                || report
                    .pending
                    .as_ref()
                    .is_some_and(|pending| pending.error.is_some())
        }) {
            bail!("档案显示试用存在错误，请先恢复本次显示变更");
        }
        state.report.as_mut().unwrap().advance()?;
        let mut runtime = runtime;
        runtime.devices.display_owner = Some(id);
        let reviewed = Reviewed {
            permission: pending.permission.as_ref(),
            dir: &pending.dir,
            file: &pending.file,
            mode: pending.mode.as_ref(),
            display: None,
            gpu_power: pending.gpu_power.as_ref(),
            graphics_mode: pending.graphics_mode.as_ref(),
        };
        match review::display_action(
            runtime.hal,
            runtime.gpu,
            runtime.devices,
            reviewed,
            DisplayAction::Keep(id),
        ) {
            Ok(_) => {
                self.remainder(&mut state, &runtime, reviewed, true)?;
            }
            Err(error) => {
                state
                    .report
                    .as_mut()
                    .unwrap()
                    .steps
                    .push(step("保留显示与重新预检", Err(error)));
                self.finish_display(&mut state, &runtime, id, false)?;
            }
        }
        Ok(state.report.clone().expect("已有应用结果"))
    }
    fn remainder(
        &self,
        state: &mut State,
        runtime: &Runtime<'_>,
        reviewed: Reviewed<'_>,
        retained: bool,
    ) -> Result<()> {
        let result = review::apply(runtime.hal, runtime.gpu, runtime.devices, reviewed);
        let report = state.report.as_mut().expect("已有应用结果");
        match result {
            Ok(outcome) => {
                report.targets_applied = outcome.targets_applied;
                report.steps.extend(outcome.steps);
            }
            Err(error) => report.steps.push(step("档案执行预检", Err(error))),
        }
        if retained {
            let success = report.targets_applied;
            let id = report.id;
            self.finish_display(state, runtime, id, success)?;
        } else {
            state.pending = None;
        }
        Ok(())
    }
    fn finish_display(
        &self,
        state: &mut State,
        runtime: &Runtime<'_>,
        id: u64,
        success: bool,
    ) -> Result<()> {
        let _gate = runtime.hal.begin_write()?;
        let result = if success {
            runtime.devices.display.release_profile(id)
        } else {
            runtime.devices.display.cancel_profile(id)
        };
        state.report.as_mut().unwrap().steps.push(step(
            if success {
                "保留档案显示"
            } else {
                "恢复本次显示变更"
            },
            result,
        ));
        self.update_pending(state, runtime.devices.display)
    }
    fn update_pending(&self, state: &mut State, display: &display::Controller) -> Result<()> {
        let id = state.report.as_ref().expect("已有应用结果").id;
        let current = display.profile_pending(id, display::now_ms())?;
        if current.is_none() {
            state.pending = None;
        }
        state.report.as_mut().unwrap().pending = current;
        Ok(())
    }
    pub(super) fn cancel(&self, runtime: Runtime<'_>, id: u64) -> Result<Application> {
        let mut state = self.lock()?;
        Self::refresh(&mut state, runtime.devices.display)?;
        if state.pending.as_ref().map(|pending| pending.id) != Some(id) {
            bail!("档案显示会话已结束或不属于此请求");
        }
        state.report.as_mut().unwrap().advance()?;
        state.report.as_mut().unwrap().targets_applied = false;
        state.report.as_mut().unwrap().steps.push(step(
            "取消档案应用",
            Err(anyhow!("已取消；其余步骤是否执行以结果为准")),
        ));
        self.finish_display(&mut state, &runtime, id, false)?;
        Ok(state.report.clone().unwrap())
    }
}
