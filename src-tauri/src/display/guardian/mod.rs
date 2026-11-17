//! 显示恢复进程协议；输入来自继承管道，不经过命令行或持久文件。
pub mod client;
pub mod protocol;
const MESSAGE_LIMIT: usize = 128 * 1024;
mod recovery;
pub(super) use recovery::{matches, restore};
#[cfg(test)]
mod tests;
use crate::display::Mode;
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Change {
    pub before: Mode,
    pub target: Mode,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Plan {
    pub changes: Vec<Change>,
    pub timeout_ms: u64,
}
impl Plan {
    pub fn validate(&self) -> Result<()> {
        if self.changes.is_empty() || self.changes.len() > 32 {
            bail!("显示试用须包含1–32个不同显示器");
        }
        let mut devices = std::collections::BTreeSet::new();
        for change in &self.changes {
            change.validate()?;
            if change.is_rotation() && self.changes.len() != 1 {
                bail!("旋转试用只允许一个主显示器");
            }
            if !devices.insert(&change.before.device) {
                bail!("显示试用不能重复配置同一显示器");
            }
        }
        if !(1..=15_000).contains(&self.timeout_ms) {
            bail!("显示试用期限无效");
        }
        if serde_json::to_vec(self)?.len() > MESSAGE_LIMIT - 64 {
            bail!("显示试用计划过大");
        }
        Ok(())
    }
    pub fn before(&self) -> Vec<Mode> {
        self.changes
            .iter()
            .map(|change| change.before.clone())
            .collect()
    }
    pub fn targets(&self) -> Vec<Mode> {
        self.changes
            .iter()
            .map(|change| change.target.clone())
            .collect()
    }
}
impl Change {
    pub(super) fn is_rotation(&self) -> bool {
        self.before
            .oem_rotation()
            .is_ok_and(|target| target == self.target)
    }
    fn validate(&self) -> Result<()> {
        let mut target = self.target.clone();
        target.frequency = self.before.frequency;
        let refresh = target == self.before && self.target.frequency != self.before.frequency;
        if !refresh && !self.is_rotation() {
            bail!("显示监护只接受刷新率或原厂主屏旋转变更");
        }
        for mode in [&self.before, &self.target] {
            mode.validate()?;
        }
        Ok(())
    }
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "camelCase", deny_unknown_fields)]
pub enum Input {
    Prepare { plan: Plan },
    Arm,
    Keep { expected: Vec<Mode> },
    Restored { expected: Vec<Mode> },
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "camelCase", deny_unknown_fields)]
pub enum Output {
    Ready,
    Armed,
    Kept,
    Released,
    Recovered,
    Rejected { message: String },
    RecoveryFailed { message: String },
}
pub trait Monitor: Send {
    fn keep(&mut self, expected: &[Mode]) -> Result<()>;
    fn restored(&mut self, expected: &[Mode]) -> Result<()>;
    fn recovered(&mut self) -> Result<bool>;
}
pub trait Factory: Send + Sync {
    fn start(&self, plan: Plan) -> Result<Box<dyn Monitor>>;
}
pub struct ProcessFactory;
impl Factory for ProcessFactory {
    fn start(&self, plan: Plan) -> Result<Box<dyn Monitor>> {
        let command = std::process::Command::new(std::env::current_exe()?);
        Ok(Box::new(client::Client::spawn(command, plan)?))
    }
}
#[cfg(test)]
pub struct MemoryFactory;
#[cfg(test)]
impl Factory for MemoryFactory {
    fn start(&self, _: Plan) -> Result<Box<dyn Monitor>> {
        Ok(Box::new(MemoryMonitor))
    }
}
#[cfg(test)]
struct MemoryMonitor;
#[cfg(test)]
impl Monitor for MemoryMonitor {
    fn keep(&mut self, _: &[Mode]) -> Result<()> {
        Ok(())
    }
    fn restored(&mut self, _: &[Mode]) -> Result<()> {
        Ok(())
    }
    fn recovered(&mut self) -> Result<bool> {
        Ok(false)
    }
}
