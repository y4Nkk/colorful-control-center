use crate::hal::screen_brightness::Reading;
use anyhow::{Result, bail};
use ccc_core::display_brightness::Targets;
use serde::Serialize;

#[derive(Clone)]
pub(super) struct Snapshot {
    pub reading: Reading,
    pub plan: u128,
    pub levels: [u32; 2],
    pub targets: Targets,
}
impl Snapshot {
    pub fn validate(&self) -> Result<()> {
        self.reading.validate()?;
        if self.levels.iter().any(|value| *value > 100) || self.targets.0.iter().any(|v| *v > 100) {
            bail!("屏幕计划亮度或原厂目标超出百分比范围");
        }
        Ok(())
    }
    pub fn identity(&self) -> Result<String> {
        self.validate()?;
        super::super::model::digest(&(
            "screen-brightness-policy-1",
            &self.reading.panel,
            self.plan,
            &self.targets,
        ))
    }
    pub fn view(&self) -> Current {
        Current {
            reading: self.reading.clone(),
            plan: format!("{:032x}", self.plan),
            levels: self.levels,
            targets: self.targets.0,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Current {
    pub reading: Reading,
    pub plan: String,
    pub levels: [u32; 2],
    pub targets: [u8; 11],
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Initial {
    pub plan: String,
    pub original: [u32; 2],
    pub owned: [bool; 2],
    pub current: Option<[u32; 2]>,
    pub error: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Profile {
    pub identity: Option<String>,
    pub current: Option<Current>,
    pub error: Option<String>,
    pub initial: Vec<Initial>,
    pub restore_revision: Option<String>,
    pub restore_error: Option<String>,
}
#[derive(Clone, Serialize)]
pub(super) struct Baseline {
    pub panel: String,
    pub levels: [u32; 2],
    pub owned: [bool; 2],
}
pub(super) struct Job {
    pub identity: String,
    pub panel: String,
    pub plan: u128,
    pub target: u8,
    pub before: Option<[u32; 2]>,
    pub owned: [bool; 2],
    pub needed: bool,
}
impl Job {
    pub fn new(snapshot: &Snapshot, expected: &str, event: u8) -> Result<Self> {
        if snapshot.identity()? != expected {
            bail!("屏幕亮度面板、计划或目标表变化，请重新确认范围");
        }
        let target = snapshot
            .targets
            .for_event(event)
            .map_err(anyhow::Error::msg)?;
        Ok(Self {
            identity: expected.into(),
            panel: snapshot.reading.panel.clone(),
            plan: snapshot.plan,
            target,
            before: None,
            owned: [false; 2],
            needed: !snapshot.reading.on_ac && snapshot.reading.current > target,
        })
    }
}
