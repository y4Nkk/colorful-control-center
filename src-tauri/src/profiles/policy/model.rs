use crate::{
    powerctl::mode::profile::ApprovalPreview,
    profiles::{ProfileFile, application::Origin},
};
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Trigger {
    Ac,
    Battery,
    Resume,
}
impl Trigger {
    pub fn label(self) -> &'static str {
        match self {
            Self::Ac => "插电",
            Self::Battery => "电池",
            Self::Resume => "唤醒",
        }
    }
    pub fn index(self) -> usize {
        match self {
            Self::Ac => 0,
            Self::Battery => 1,
            Self::Resume => 2,
        }
    }
    pub fn origin(self) -> Origin {
        match self {
            Self::Ac => Origin::Ac,
            Self::Battery => Origin::Battery,
            Self::Resume => Origin::Resume,
        }
    }
}

pub(super) use crate::profiles::authorization::Binding;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Record {
    pub version: u8,
    pub enabled: bool,
    pub ac: Option<Binding>,
    pub battery: Option<Binding>,
    pub resume: Option<Binding>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Selection {
    pub file: ProfileFile,
    pub mode: Option<ApprovalPreview>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Request {
    pub expected: Record,
    pub enabled: bool,
    pub ac: Option<Selection>,
    pub battery: Option<Selection>,
    pub resume: Option<Selection>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct View {
    pub record: Option<Record>,
    pub initial: Option<Record>,
    pub paused: bool,
    pub active: bool,
    pub source: Option<crate::hal::power_events::Source>,
    pub sleeping: bool,
    pub pending: Option<Trigger>,
    pub route: Option<crate::hal::power_events::Route>,
    pub configuration_error: Option<String>,
    pub reset_revision: Option<String>,
    pub listener_error: Option<String>,
    pub last_issue: Option<String>,
}
impl Default for Record {
    fn default() -> Self {
        Self {
            version: 2,
            enabled: false,
            ac: None,
            battery: None,
            resume: None,
        }
    }
}
impl Record {
    pub fn binding_mut(&mut self, trigger: Trigger) -> Option<&mut Binding> {
        match trigger {
            Trigger::Ac => self.ac.as_mut(),
            Trigger::Battery => self.battery.as_mut(),
            Trigger::Resume => self.resume.as_mut(),
        }
    }
    pub fn binding(&self, trigger: Trigger) -> Option<&Binding> {
        match trigger {
            Trigger::Ac => self.ac.as_ref(),
            Trigger::Battery => self.battery.as_ref(),
            Trigger::Resume => self.resume.as_ref(),
        }
    }
    pub fn rules(&self) -> [bool; 3] {
        [
            self.ac.is_some(),
            self.battery.is_some(),
            self.resume.is_some(),
        ]
    }
    pub fn validate(&self) -> Result<()> {
        if self.version != 2 {
            bail!("不支持的自动档案策略版本 {}", self.version);
        }
        if self.enabled && self.rules().iter().all(|b| !b) {
            bail!("启用自动档案前至少选择一条规则");
        }
        for binding in [&self.ac, &self.battery, &self.resume]
            .into_iter()
            .flatten()
        {
            binding.validate()?;
        }
        Ok(())
    }
}
