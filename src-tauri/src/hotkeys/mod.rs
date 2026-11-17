mod actions;
mod battery_alert;
mod battery_cold;
mod battery_thresholds;
mod camera_report;
mod low_battery_action;
mod model;
mod notifications;
mod panel_hotkey;
mod panel_event;
mod runtime;
mod screen_brightness;
mod store;
mod system_action;
mod system_recovery;
#[cfg(test)]
mod tests;
mod transaction;
use anyhow::{Context, Result, anyhow};
use model::*;
pub use model::{Request, Settings, Status};
use serde::Serialize;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tauri::{AppHandle, Emitter, Manager};

pub struct Controller {
    battery_thresholds: battery_thresholds::Controller,
    battery_alert: battery_alert::Controller,
    panel_hotkey: panel_hotkey::Controller,
    screen_brightness: screen_brightness::Controller,
    path: PathBuf,
    mutation: Mutex<Option<Record>>,
    session: Mutex<Option<runtime::Session>>,
    status: Arc<Mutex<Status>>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    revision: Option<String>,
    current: Option<Settings>,
    record_error: Option<String>,
    reset_revision: Option<String>,
    initial: Option<Settings>,
    facts: Facts,
    authorized: bool,
    status: Status,
    osd_error: Option<String>,
}
struct Native<'a> {
    controller: &'a Controller,
    app: &'a AppHandle,
}
impl transaction::Io for Native<'_> {
    type Prepared = runtime::Session;
    fn load(&self) -> Result<store::Loaded> {
        store::load(&self.controller.path)
    }
    fn facts(&self) -> Facts {
        actions::facts(self.app, &SCOPES)
    }
    fn stop(&self) -> Result<bool> {
        self.controller.stop_runtime(self.app)
    }
    fn prepare(&self, record: &Record) -> Result<Self::Prepared> {
        record.verify(&actions::facts(self.app, &record.settings.actions))?;
        runtime::Session::prepare(
            self.app.clone(),
            self.controller.path.clone(),
            record.clone(),
            self.controller.status.clone(),
        )
    }
    fn activate(&self, mut prepared: Self::Prepared) -> Result<()> {
        prepared.activate()?;
        *self
            .controller
            .session
            .lock()
            .map_err(|_| anyhow!("热键监听状态异常"))? = Some(prepared);
        Ok(())
    }
    fn save(&self, before: &store::Loaded, target: &Record) -> Result<store::Loaded> {
        store::replace(&self.controller.path, before, target)
    }
}
impl Controller {
    pub fn status(&self) -> Result<model::Status> {
        Ok(self
            .status
            .lock()
            .map_err(|_| anyhow!("热键运行状态异常"))?
            .clone())
    }
    pub fn dismiss_cold_warning(&self, app: &AppHandle, expected_id: u64) -> Result<()> {
        let mut status = self
            .status
            .lock()
            .map_err(|_| anyhow!("热键运行状态异常"))?;
        let previous = status.cold_warning.clone();
        status.dismiss_cold_warning(expected_id)?;
        if let Err(error) = app.emit_to("main", "hotkeys-state", status.clone()) {
            status.cold_warning = previous;
            return Err(error).context("发送低温提醒关闭状态");
        }
        Ok(())
    }
    pub fn acknowledge_navigation(&self, app: &AppHandle, expected_id: u64) -> Result<()> {
        let mut status = self
            .status
            .lock()
            .map_err(|_| anyhow!("热键运行状态异常"))?;
        let previous = status.navigation_request.clone();
        status.acknowledge_navigation(expected_id)?;
        if let Err(error) = app.emit_to("main", "hotkeys-state", status.clone()) {
            status.navigation_request = previous;
            return Err(error).context("发送热键导航处理状态");
        }
        Ok(())
    }
    pub fn new(path: PathBuf) -> Self {
        Self {
            battery_alert: battery_alert::Controller::new(
                path.with_file_name("oem-battery-alert-audio.json"),
            ),
            battery_thresholds: battery_thresholds::Controller::new(
                path.with_file_name("oem-battery-thresholds.json"),
            ),
            panel_hotkey: panel_hotkey::Controller::default(),
            screen_brightness: screen_brightness::Controller::new(
                path.with_file_name("oem-low-battery-brightness.json"),
            ),
            path,
            mutation: Mutex::new(None),
            session: Mutex::new(None),
            status: Arc::new(Mutex::new(Status::default())),
        }
    }
    pub fn read(&self, app: &AppHandle) -> Result<Profile> {
        let initial = self
            .mutation
            .lock()
            .map_err(|_| anyhow!("热键策略状态异常"))?
            .as_ref()
            .map(|record| record.settings.clone());
        let inspected = store::inspect(&self.path)?;
        let facts = actions::facts(app, &SCOPES);
        let revision = inspected
            .loaded
            .as_ref()
            .map(|loaded| transaction::revision(loaded, &facts))
            .transpose()?;
        Ok(Profile {
            revision,
            authorized: inspected
                .loaded
                .as_ref()
                .is_some_and(|loaded| loaded.record.verify(&facts).is_ok()),
            current: inspected.loaded.map(|loaded| loaded.record.settings),
            record_error: inspected.error,
            reset_revision: inspected.reset_revision,
            initial,
            facts,
            status: self
                .status
                .lock()
                .map_err(|_| anyhow!("热键运行状态异常"))?
                .clone(),
            osd_error: app.state::<crate::osd::Controller>().error()?,
        })
    }
    pub fn apply(&self, app: &AppHandle, request: Request) -> Result<()> {
        let mut baseline = self
            .mutation
            .lock()
            .map_err(|_| anyhow!("热键策略状态异常"))?;
        transaction::change(
            &Native {
                controller: self,
                app,
            },
            &request.expected_revision,
            |facts| Record::authorize(request.settings, facts),
            &mut baseline,
        )
    }
    pub fn restore(&self, app: &AppHandle, expected: &str) -> Result<()> {
        let mut baseline = self
            .mutation
            .lock()
            .map_err(|_| anyhow!("热键策略状态异常"))?;
        let initial = baseline.clone().context("尚未修改热键软件策略")?;
        transaction::change(
            &Native {
                controller: self,
                app,
            },
            expected,
            |_| Ok(initial),
            &mut baseline,
        )
    }
    pub fn stop(&self, app: &AppHandle) -> Result<bool> {
        let _mutation = self
            .mutation
            .lock()
            .map_err(|_| anyhow!("热键策略状态异常"))?;
        self.stop_runtime(app)
    }
    pub fn reset_invalid(&self, app: &AppHandle, expected: &str) -> Result<()> {
        let mut baseline = self
            .mutation
            .lock()
            .map_err(|_| anyhow!("热键策略状态异常"))?;
        let inspected = store::inspect(&self.path)?;
        if inspected.loaded.is_some() || inspected.reset_revision.as_deref() != Some(expected) {
            anyhow::bail!("不可用热键策略已变化，请重新查看后确认清除");
        }
        self.stop_runtime(app)?;
        store::reset(&self.path, expected)?;
        *baseline = None;
        Ok(())
    }
    pub fn restore_screen_brightness(&self, app: &AppHandle, expected: &str) -> Result<()> {
        let _mutation = self
            .mutation
            .lock()
            .map_err(|_| anyhow!("热键策略状态异常"))?;
        let preview = self
            .screen_brightness
            .read(&app.state::<crate::hal::Hal>())?;
        if preview.restore_revision.as_deref() != Some(expected) {
            anyhow::bail!("初始屏幕计划亮度预览已变化，请重新查看");
        }
        self.stop_runtime(app)?;
        self.screen_brightness
            .restore(&app.state::<crate::hal::Hal>(), expected)
    }
    pub fn restore_battery_alert_audio(&self, app: &AppHandle, expected: &str) -> Result<()> {
        let _mutation = self
            .mutation
            .lock()
            .map_err(|_| anyhow!("热键策略状态异常"))?;
        let hal = app.state::<crate::hal::Hal>();
        let before = self.battery_alert.audio(&hal, &self.screen_brightness)?;
        anyhow::ensure!(
            before.restore_revision.as_deref() == Some(expected),
            "电池音频恢复预览已变化"
        );
        let stopped = self.stop_runtime(app);
        let after = self.battery_alert.audio(&hal, &self.screen_brightness)?;
        if after.initial.is_none() && after.current == before.initial {
            return Ok(());
        }
        let next = after.restore_revision.as_deref().with_context(|| {
            after
                .restore_error
                .or(after.error)
                .unwrap_or_else(|| "电池音频恢复不可用".into())
        })?;
        match self
            .battery_alert
            .restore(&hal, &self.screen_brightness, next)
        {
            Ok(()) => Ok(()),
            Err(error) => anyhow::bail!(ccc_core::msr::with_recovery(
                format!("{error:#}"),
                stopped
                    .map(|_| ())
                    .map_err(|error| format!("停止监听：{error:#}"))
            )),
        }
    }
    pub fn restore_low_battery_brightness(&self, app: &AppHandle, expected: &str) -> Result<()> {
        let _mutation = self
            .mutation
            .lock()
            .map_err(|_| anyhow!("热键策略状态异常"))?;
        let hal = app.state::<crate::hal::Hal>();
        let preview = self.screen_brightness.read_low(&hal)?;
        if preview.restore_revision.as_deref() != Some(expected) {
            anyhow::bail!("低电量亮度恢复预览已变化，请重新查看");
        }
        self.stop_runtime(app)?;
        self.screen_brightness.restore_low(&hal, expected)
    }
    pub fn restore_gpu_turbo(&self, app: &AppHandle, expected: &str) -> Result<()> {
        let _mutation = self
            .mutation
            .lock()
            .map_err(|_| anyhow!("热键策略状态异常"))?;
        let gpu = app.state::<crate::gpuctl::GpuController>();
        let hal = app.state::<crate::hal::Hal>();
        let preview = gpu.read_turbo(&hal)?;
        if preview.restore_revision.as_deref() != Some(expected) {
            anyhow::bail!("GPU Turbo 初始恢复预览已变化，请重新查看");
        }
        self.stop_runtime(app)?;
        gpu.restore_turbo(&hal, expected)
    }
    pub fn restore_battery_thresholds(&self, app: &AppHandle, expected: &str) -> Result<()> {
        let _mutation = self
            .mutation
            .lock()
            .map_err(|_| anyhow!("热键策略状态异常"))?;
        let hal = app.state::<crate::hal::Hal>();
        let profile = self.battery_thresholds.read(&hal)?;
        if profile.restore_revision.as_deref() != Some(expected) {
            anyhow::bail!("电池阈值初始恢复预览已变化，请重新查看");
        }
        self.stop_runtime(app)?;
        self.battery_thresholds.restore(&hal, expected)
    }
    pub fn restore_pd_event(&self, app: &AppHandle, expected: &str) -> Result<()> {
        let _mutation = self
            .mutation
            .lock()
            .map_err(|_| anyhow!("热键策略状态异常"))?;
        let hal = app.state::<crate::hal::Hal>();
        let controller = app.state::<crate::platform::Controller>();
        let profile = controller.read_pd_event(&hal)?;
        let current = profile.current.context("PD恢复状态不可用")?;
        if current.revision != expected
            || current.initial.is_none()
            || current.blocked_reason.is_some()
            || current.restore_blocked_reason.is_some()
        {
            anyhow::bail!("PD初始恢复预览已变化或不可执行，请重新查看");
        }
        self.stop_runtime(app)?;
        controller.restore(&hal, expected)
    }
    fn stop_runtime(&self, app: &AppHandle) -> Result<bool> {
        let session = self
            .session
            .lock()
            .map_err(|_| anyhow!("热键监听状态异常"))?
            .take();
        let running = session.is_some();
        let retry_audio = session.as_ref().is_none_or(runtime::Session::finished);
        let stopped = session.map(runtime::Session::stop).transpose();
        let panel = self.panel_hotkey.end(None);
        let audio = if retry_audio {
            self.battery_alert
                .cleanup(&app.state::<crate::hal::Hal>(), &self.screen_brightness)
        } else {
            Ok(())
        };
        self.status
            .lock()
            .map_err(|_| anyhow!("热键运行状态异常"))?
            .running = false;
        let errors: Vec<_> = [stopped.map(|_| ()), panel.map(|_| ()), audio]
            .into_iter()
            .filter_map(Result::err)
            .map(|error| format!("{error:#}"))
            .collect();
        if !errors.is_empty() {
            anyhow::bail!("{}", errors.join("；"));
        }
        Ok(running)
    }
    pub fn end_panel_hotkey(&self, app: &AppHandle, expected: &str) -> Result<()> {
        let _mutation = self
            .mutation
            .lock()
            .map_err(|_| anyhow!("热键策略状态异常"))?;
        let _frozen = self.panel_hotkey.freeze(expected)?;
        self.stop_runtime(app)?;
        Ok(())
    }
    pub fn resume_saved(&self, app: &AppHandle) {
        let result = (|| -> Result<()> {
            let _mutation = self
                .mutation
                .lock()
                .map_err(|_| anyhow!("热键策略状态异常"))?;
            let loaded = store::load(&self.path)?;
            if !loaded.record.settings.enabled {
                return Ok(());
            }
            use transaction::Io;
            let native = Native {
                controller: self,
                app,
            };
            native.stop()?;
            native.activate(native.prepare(&loaded.record)?)
        })();
        if let Err(error) = result {
            if let Ok(mut state) = self.status.lock() {
                state.error = Some(format!("恢复热键监听失败：{error:#}"));
            }
            log::error!("[hotkeys] {error:#}");
        }
    }
}
