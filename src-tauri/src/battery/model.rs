use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChargeValues {
    pub enabled: bool,
    pub start: u8,
    pub stop: u8,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChargeState {
    pub current: ChargeValues,
    pub start_values: Vec<u8>,
    pub stop_values: Vec<u8>,
}
impl ChargeState {
    pub fn validate_snapshot(&self) -> Result<()> {
        if self.current.start > 100 || self.current.stop > 100 {
            bail!("充电快照阈值无效");
        }
        for values in [&self.start_values, &self.stop_values] {
            let mut unique = values.to_vec();
            unique.sort_unstable();
            unique.dedup();
            if values.is_empty()
                || values.len() > 16
                || unique.len() != values.len()
                || values.iter().any(|v| *v == 0 || *v > 100)
            {
                bail!("充电快照候选无效");
            }
        }
        Ok(())
    }
    pub fn revision(&self) -> String {
        format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(self).expect("固定充电结构"))
        )
    }
    pub fn same_capabilities(&self, other: &Self) -> Result<()> {
        if self.start_values != other.start_values || self.stop_values != other.stop_values {
            bail!("充电阈值候选已变化，未执行旧设置");
        }
        Ok(())
    }
    pub fn target(&self, values: &ChargeValues) -> Result<Self> {
        let preserve_disabled = !values.enabled
            && values.start == self.current.start
            && values.stop == self.current.stop;
        if !preserve_disabled
            && (!self.start_values.contains(&values.start)
                || !self.stop_values.contains(&values.stop)
                || values.start >= values.stop)
        {
            bail!("请选择固件支持且开始低于停止的充电阈值");
        }
        Ok(Self {
            current: values.clone(),
            ..self.clone()
        })
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChargeRequest {
    pub expected_revision: String,
    pub values: ChargeValues,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChargeTarget {
    pub values: ChargeValues,
    pub start_values: Vec<u8>,
    pub stop_values: Vec<u8>,
}
impl ChargeTarget {
    pub fn state(&self) -> ChargeState {
        ChargeState {
            current: self.values.clone(),
            start_values: self.start_values.clone(),
            stop_values: self.stop_values.clone(),
        }
    }
    pub fn validate(&self) -> Result<()> {
        let state = self.state();
        state.validate_snapshot()?;
        state.target(&self.values)?;
        Ok(())
    }
    pub fn capture(state: ChargeState) -> Self {
        Self {
            values: state.current,
            start_values: state.start_values,
            stop_values: state.stop_values,
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PowerStatus {
    pub present: Option<bool>,
    pub ac_online: Option<bool>,
    pub charging: Option<bool>,
    pub percent: Option<u8>,
    pub remaining_seconds: Option<u32>,
}
/// 充电保护读取失败。`reason` 面向用户，`Display` 连同技术细节进入错误链。
#[derive(Debug)]
pub enum ChargeUnavailable {
    /// 原厂控制组件缺失、缺少导出，或没有双向包传输。
    Component(anyhow::Error),
    /// 原厂通道调用失败，或返回的包不完整。
    Transport(anyhow::Error),
    /// 固件返回的充电保护数据无法识别。
    Firmware(anyhow::Error),
}
impl ChargeUnavailable {
    pub fn reason(&self) -> &'static str {
        match self {
            Self::Component(_) => "未找到可用的原厂控制组件，无法读取充电保护设置。",
            Self::Transport(_) => "原厂通道没有响应，无法读取充电保护设置。",
            Self::Firmware(_) => "本机固件不支持充电保护，或返回的数据无法识别。",
        }
    }
    fn kind(&self) -> (&'static str, &anyhow::Error) {
        match self {
            Self::Component(error) => ("原厂控制组件不可用", error),
            Self::Transport(error) => ("原厂通道无响应", error),
            Self::Firmware(error) => ("固件充电保护数据无法识别", error),
        }
    }
}
impl std::fmt::Display for ChargeUnavailable {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (kind, error) = self.kind();
        write!(formatter, "{kind}：{error:#}")
    }
}
impl std::error::Error for ChargeUnavailable {}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostics {
    pub version: u16,
    pub manufactured_on: Option<String>,
    pub cycle_count: u16,
    pub full_capacity_raw: u16,
    pub design_capacity_raw: u16,
    pub capacity_pct: Option<f64>,
    pub battery_status: u16,
    pub permanent_failure_status: u32,
    pub operation_status: u32,
    pub stop_charging_threshold: u8,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub charge: Option<ChargeState>,
    pub revision: Option<String>,
    pub power: Option<PowerStatus>,
    pub diagnostics: Option<Diagnostics>,
    /// 充电保护读取失败的原因；与 charge 同时为空表示固件未提供该功能。
    pub charge_unavailable_reason: Option<String>,
    /// Windows 电源状态读取失败的原因。
    pub power_unavailable_reason: Option<String>,
    pub charge_lock_reason: Option<String>,
}
