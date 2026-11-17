use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub automatic_power_off: bool,
    pub problem: u32,
    pub drives_display: bool,
    pub can_disable: bool,
}
impl Device {
    pub fn target(&self) -> Target {
        Target {
            enabled: self.enabled,
            automatic_power_off: self.automatic_power_off,
        }
    }
    pub fn healthy(&self) -> bool {
        self.problem == if self.enabled { 0 } else { 22 }
    }
    // 冻结阶段的前提约束；执行前仍须核对完整设备实读。
    pub(crate) fn projected(&self, target: Target) -> Self {
        Self {
            enabled: target.enabled,
            automatic_power_off: target.automatic_power_off,
            problem: if target.enabled { 0 } else { 22 },
            ..self.clone()
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Target {
    pub enabled: bool,
    pub automatic_power_off: bool,
}
impl Target {
    pub fn requested(enabled: bool) -> Self {
        Self {
            enabled,
            automatic_power_off: !enabled,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Plan {
    pub id: String,
    pub expected: Target,
    pub target: Target,
}
impl Plan {
    pub fn validate(&self) -> Result<()> {
        if !self.id.starts_with("ACPI\\NVDA0820\\")
            || self.id.len() <= "ACPI\\NVDA0820\\".len()
            || self.id.split('\\').count() != 3
            || self.id.len() > 512
            || !self
                .id
                .bytes()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || b"\\&_-{}".contains(&c))
        {
            bail!("原厂独显控制实例无效");
        }
        Ok(())
    }
}
