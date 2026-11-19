//! FlexiKey保存意图、原厂来源和持久恢复；过滤器没有公开映射表getter。
#[path = "keymap/file.rs"]
pub mod file;
#[path = "keymap/hotkey.rs"]
mod hotkey;
#[path = "keymap/launch.rs"]
mod launch;
#[path = "keymap/lifecycle.rs"]
mod lifecycle;
#[path = "keymap/model.rs"]
mod model;
#[cfg(not(test))]
#[path = "keymap/native.rs"]
mod native;
#[path = "keymap/original.rs"]
pub mod original;
#[path = "keymap/recording.rs"]
pub mod recording;
#[path = "keymap/scope.rs"]
mod scope;
#[path = "keymap/store.rs"]
mod store;
#[cfg(test)]
#[path = "keymap/tests.rs"]
mod tests;
#[path = "keymap/transaction.rs"]
mod transaction;
pub(crate) use self::scope::Scope;
use crate::hal::Hal;
use anyhow::{Context, Result, anyhow, bail};
#[cfg(test)]
use ccc_core::keymap;
use ccc_core::keymap::{Configuration, Device};
pub(crate) use launch::Session;
#[cfg(test)]
pub(crate) use launch::TestPrograms;
pub(crate) use model::Record;
pub use model::Target;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
pub(crate) use transaction::Plan;

#[derive(Clone)]
pub(crate) struct Facts {
    pub capability: [u8; 15],
    pub flags: [u8; 2],
    pub original: Option<original::Source>,
    pub original_error: Option<String>,
}
pub(crate) trait Io {
    fn facts(&self) -> Result<Facts>;
    fn ensure_available(&self) -> Result<()>;
    fn replace(&self, target: &Configuration) -> Result<()>;
    fn save_flags(&self, flags: [u8; 2], scope: &[Device]) -> Result<()>;
    fn prepare_programs(&self, record: &Record, gate: Arc<Mutex<()>>) -> Result<Option<Session>>;
}
fn supported(bytes: &[u8; 15]) -> Result<bool> {
    match &bytes[..2] {
        [0, 0] => Ok(false),
        [1, 0] => Ok(bytes[13] & 2 != 0),
        _ => bail!("未知FlexiKey能力页版本"),
    }
}
#[derive(Clone)]
struct Frame {
    facts: Facts,
    loaded: store::Loaded,
    blocked: Option<String>,
}
impl Frame {
    fn current(&self) -> Option<&Configuration> {
        self.loaded
            .record
            .as_ref()
            .map(|r| &r.current)
            .or_else(|| self.facts.original.as_ref().map(|s| &s.configuration))
    }
    fn revision(&self) -> Result<String> {
        Ok(format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&(
                &self.facts.capability,
                self.facts.flags,
                self.facts.original.as_ref().map(|s| &s.fingerprint),
                &self.facts.original_error,
                &self.loaded.revision,
                &self.blocked
            ))?)
        ))
    }
    fn review(&self, expected: &str, restoring: bool) -> Result<()> {
        if self.revision()? != expected {
            bail!("宏配置预览已变化，请重新读取并确认");
        }
        if !supported(&self.facts.capability)? {
            bail!("固件未声明FlexiKey能力");
        }
        if let Some(reason) = &self.blocked {
            bail!(reason.clone());
        }
        if !restoring
            && self
                .loaded
                .record
                .as_ref()
                .is_some_and(|r| r.phase == model::Phase::Prepared)
        {
            bail!("上次宏操作尚需恢复，请先恢复初始配置");
        }
        Ok(())
    }
}
fn capture(io: &(impl Io + ?Sized), store: &impl store::Store) -> Result<Frame> {
    let facts = io.facts()?;
    if facts.flags.iter().any(|v| *v > 1) {
        bail!("原厂过滤启用状态无效");
    }
    supported(&facts.capability)?;
    let loaded = store.load()?;
    Ok(Frame {
        facts,
        loaded,
        blocked: io.ensure_available().err().map(|e| format!("{e:#}")),
    })
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    revision: String,
    configuration: Option<Configuration>,
    enabled_flags: [u8; 2],
    source_name: String,
    baseline_name: Option<String>,
    can_restore: bool,
    recovery_required: bool,
    blocked_reason: Option<String>,
    warnings: Vec<String>,
    programs_required: bool,
    programs: launch::Status,
}
fn read(io: &(impl Io + ?Sized), store: &impl store::Store) -> Result<Option<Profile>> {
    let frame = capture(io, store)?;
    if !supported(&frame.facts.capability)? {
        if frame.loaded.record.is_some() {
            bail!("当前固件未声明FlexiKey能力，原恢复记录已保留；能力恢复后可重新读取并恢复");
        }
        return Ok(None);
    }
    let record = frame.loaded.record.as_ref();
    let mut warnings = vec!["此处显示保存的配置及驱动接受结果，过滤器未提供映射表回读。".into()];
    if record.is_none() && frame.facts.original.is_none() {
        warnings.push("原厂活动配置不存在，无法建立映射恢复基线。".into());
    }
    if let Some(error) = &frame.facts.original_error {
        warnings.push(error.clone());
    }
    if frame.current().is_some_and(|configuration| {
        configuration.devices.iter().any(|device| {
            u8::from(device.enabled) != frame.facts.flags[model::index(device.device)]
        })
    }) {
        warnings.push("配置启用意图与原厂启用记录不同；应用时将按预览更新所选设备。".into());
    }
    let recovery_required = record.is_some_and(|r| r.phase == model::Phase::Prepared);
    Ok(Some(Profile {
        revision: frame.revision()?,
        configuration: frame.current().cloned(),
        enabled_flags: frame.facts.flags,
        source_name: record.map_or_else(
            || {
                frame
                    .facts
                    .original
                    .as_ref()
                    .map_or("未读取到原厂活动配置".into(), |s| s.name.clone())
            },
            |r| {
                if r.phase == model::Phase::Prepared {
                    "上次操作目标（执行结果待恢复）".into()
                } else {
                    "本应用保存的已应用配置".into()
                }
            },
        ),
        baseline_name: record.map(|r| r.baseline.name.clone()),
        can_restore: record.is_some(),
        recovery_required,
        blocked_reason: frame.blocked,
        warnings,
        programs_required: record.is_some_and(launch::required),
        programs: launch::Status::default(),
    }))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub expected_revision: String,
    pub configuration: Configuration,
}
pub struct Controller {
    path: PathBuf,
    mutation: Mutex<()>,
    recording: Arc<recording::Controller>,
    launchers: launch::Controller,
    #[cfg(test)]
    test_io: Option<Arc<dyn Io + Send + Sync>>,
}
impl Controller {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            mutation: Mutex::new(()),
            recording: Arc::new(recording::Controller::default()),
            launchers: launch::Controller::default(),
            #[cfg(test)]
            test_io: None,
        }
    }
    pub fn read(&self, hal: &Hal) -> Result<Option<Profile>> {
        let scope = self.scope()?;
        let _gate = hal.begin_write()?;
        scope.read(hal)
    }
    pub fn apply(&self, hal: &Hal, request: &Request) -> Result<()> {
        let mut scope = self.scope()?;
        let plan = {
            let _gate = hal.begin_write()?;
            scope.reviewed(hal, request)?
        };
        scope.run_single(hal, &plan)
    }
    pub fn restore(&self, hal: &Hal, expected: &str) -> Result<()> {
        let mut scope = self.scope()?;
        let plan = {
            let _gate = hal.begin_write()?;
            scope.initial(hal, expected)?
        };
        scope.run_single(hal, &plan)
    }
    pub fn start_recording(&self) -> Result<recording::Snapshot> {
        let _scope = self.scope()?;
        self.recording.start()
    }
    pub fn retry_programs(&self, hal: &Hal, expected: &str) -> Result<()> {
        let scope = self.scope()?;
        scope.retry_programs(hal, expected)
    }
    pub fn read_recording(&self) -> Result<Option<recording::Snapshot>> {
        self.recording.read()
    }
    pub fn stop_recording(&self) -> Result<Option<recording::Snapshot>> {
        self.recording.stop()
    }

    pub(crate) fn release_programs_on_exit(&self) -> Result<()> {
        self.launchers.replace(None)
    }
}
