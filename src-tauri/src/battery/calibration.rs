//! 校准任务核心；生产开始入口须由独立任务进程完成准备/接管握手后开放。
#[path = "calibration/client.rs"]
pub(crate) mod client;
#[path = "calibration/coordinator.rs"]
mod coordinator;
#[path = "calibration/engine.rs"]
mod engine;
#[path = "calibration/events.rs"]
mod events;
#[path = "calibration/model.rs"]
mod model;
#[path = "calibration/native.rs"]
mod native;
#[path = "calibration/ownership.rs"]
pub(crate) mod ownership;
#[path = "calibration/power_plan.rs"]
pub(crate) mod power_plan;
#[path = "calibration/process.rs"]
mod process;
#[path = "calibration/store.rs"]
pub(crate) mod store;
#[cfg(test)]
#[path = "calibration/tests.rs"]
mod tests;
#[path = "calibration/worker.rs"]
pub(crate) mod worker;

use super::{energy, model::ChargeState};
use anyhow::Result;
pub(super) use client::Client;
pub(crate) use coordinator::Profile;
pub(super) use coordinator::charge_lock;
pub(crate) use engine::Task;
pub(crate) use model::{Event, Facts, Phase, State, Status};
pub(in crate::battery) use power_plan::format as power_scheme;
pub(crate) use process::run_child;
pub(crate) const RECORD_NAME: &str = "battery-calibration.json";
pub(crate) fn ensure_system_plan_available(path: &std::path::Path) -> Result<()> {
    use store::Store;
    if store::FileStore(path.into())
        .load()?
        .is_some_and(|r| r.pending)
    {
        anyhow::bail!("电池校准正在接管或尚需恢复，暂不能切换电源模式及其关联策略");
    }
    Ok(())
}
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Baseline {
    pub stop_threshold: u8,
    pub power_scheme: String,
    pub charge: ChargeState,
    pub energy: Option<energy::EnergySnapshot>,
}
impl Baseline {
    pub fn validate(&self) -> Result<()> {
        power_plan::parse(&self.power_scheme)?;
        if !(1..=100).contains(&self.stop_threshold) {
            anyhow::bail!("校准快照的停止阈值无效");
        }
        self.charge.validate_snapshot()?;
        if let Some(energy) = &self.energy {
            energy.validate()?;
            if energy.charge != self.charge {
                anyhow::bail!("校准与节能的充电恢复快照不一致");
            }
        }
        Ok(())
    }
}
pub(crate) trait Access {
    fn ready(&mut self) -> Result<()>;
    fn verify_baseline(&mut self, baseline: &Baseline) -> Result<()>;
    fn facts(&mut self) -> Result<Facts>;
    fn events(&mut self) -> Result<Vec<Event>>;
    fn send(&mut self, command: i32, value: u32) -> Result<()>;
    fn keep_awake(&mut self, enabled: bool) -> Result<()>;
    fn prepare_power_plan(&mut self, before: &str) -> Result<()>;
    fn enter_power_plan(&mut self, before: &str) -> Result<()>;
    fn check_power_plan(&mut self) -> Result<()>;
    fn restore_power_plan(&mut self, before: &str) -> Result<()>;
    fn restore_policy(&mut self, baseline: &Baseline) -> Result<()>;
}
