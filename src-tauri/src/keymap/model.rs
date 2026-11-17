use anyhow::{Result, anyhow, bail};
use ccc_core::keymap::{self, Configuration, Device};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Target {
    pub configuration: Configuration,
    pub programs_enabled: bool,
}
impl Target {
    pub fn validate(&self) -> Result<()> {
        keymap::validate(&self.configuration).map_err(|e| anyhow!(e))
    }
    pub(super) fn require_program_scope(&self, record: &Record) -> Result<()> {
        if self.programs_enabled
            && super::launch::program_scope(record).iter().any(|device| {
                !self
                    .configuration
                    .devices
                    .iter()
                    .any(|target| target.device == *device)
            })
        {
            bail!("程序监听包含档案未展示的已接管设备，请捕获完整档案后重新确认");
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Baseline {
    pub name: String,
    pub configuration: Configuration,
    pub flags: [u8; 2],
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Phase {
    Prepared,
    Applied,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Record {
    pub version: u8,
    pub baseline: Baseline,
    pub current: Configuration,
    pub owned: Vec<Device>,
    pub phase: Phase,
}
pub fn index(device: Device) -> usize {
    match device {
        Device::Keyboard => 0,
        Device::Mouse => 1,
    }
}
impl Record {
    pub fn validate(&self) -> Result<()> {
        if self.version != 1 || self.baseline.flags.iter().any(|v| *v > 1) {
            bail!("宏恢复记录版本或启用状态无效");
        }
        keymap::validate(&self.baseline.configuration).map_err(|e| anyhow!(e))?;
        keymap::validate(&self.current).map_err(|e| anyhow!(e))?;
        for cfg in [&self.baseline.configuration, &self.current] {
            if cfg.devices.len() != 2 {
                bail!("宏恢复记录缺少完整的原设备配置");
            }
        }
        if self.owned.is_empty()
            || self.owned.iter().collect::<BTreeSet<_>>().len() != self.owned.len()
        {
            bail!("宏恢复设备范围无效");
        }
        Ok(())
    }
}
pub fn subset(configuration: &Configuration, scope: &[Device]) -> Result<Configuration> {
    Ok(Configuration {
        devices: scope
            .iter()
            .map(|wanted| {
                configuration
                    .devices
                    .iter()
                    .find(|d| d.device == *wanted)
                    .cloned()
                    .ok_or_else(|| anyhow!("恢复配置缺少目标设备"))
            })
            .collect::<Result<_>>()?,
    })
}
pub fn merge(before: &Configuration, target: &Configuration) -> Result<Configuration> {
    let mut result = before.clone();
    for wanted in &target.devices {
        *result
            .devices
            .iter_mut()
            .find(|d| d.device == wanted.device)
            .ok_or_else(|| anyhow!("初态没有目标设备配置"))? = wanted.clone();
    }
    keymap::validate(&result).map_err(|e| anyhow!(e))?;
    Ok(result)
}
pub fn flags(before: [u8; 2], target: &Configuration) -> [u8; 2] {
    let mut result = before;
    for device in &target.devices {
        result[index(device.device)] = u8::from(device.enabled);
    }
    result
}
