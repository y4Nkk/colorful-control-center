//! 原厂NVPCF控制节点、固件命令与自动关断策略的统一事务。
use anyhow::{Result, anyhow, bail};
use serde::Serialize;
use std::sync::Mutex;
#[path = "gpu_power/environment.rs"]
mod environment;
#[path = "gpu_power/execution.rs"]
mod execution;
#[path = "gpu_power/mode.rs"]
pub(crate) mod mode;
#[path = "gpu_power/model.rs"]
pub mod model;
#[path = "gpu_power/operation.rs"]
pub mod operation;
#[path = "gpu_power/profile.rs"]
pub mod profile;
#[path = "gpu_power/request.rs"]
pub(crate) mod request;
#[path = "gpu_power/runtime.rs"]
mod runtime;
#[path = "gpu_power/scope.rs"]
mod scope;
#[path = "gpu_power/sequence.rs"]
pub(crate) mod sequence;
pub(crate) use scope::Scope;
#[path = "gpu_power/worker.rs"]
mod worker;
pub use model::{Device, Plan, Target};
pub use worker::run_child;
#[path = "hal/gpu_device.rs"]
pub mod native;
#[cfg(test)]
#[path = "gpu_power/runtime_tests.rs"]
mod runtime_tests;
#[cfg(test)]
#[path = "gpu_power/sequence_tests.rs"]
mod sequence_tests;
#[cfg(test)]
#[path = "gpu_power/session_tests.rs"]
mod session_tests;
#[cfg(test)]
#[path = "gpu_power/test_fixture.rs"]
pub(crate) mod test_fixture;
#[cfg(test)]
#[path = "gpu_power/tests.rs"]
mod tests;

pub trait Access {
    fn list(&self) -> Result<Vec<Device>>;
    fn set(&self, expected: &Device, target: Target) -> Result<()>;
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub devices: Vec<Device>,
}
#[derive(Default)]
pub struct Controller {
    mutation: Mutex<()>,
    environment: Option<std::sync::Arc<dyn environment::Environment>>,
}
impl Controller {
    pub fn read(&self, io: &impl Access) -> Result<Profile> {
        Ok(Profile { devices: io.list()? })
    }
    pub fn apply(
        &self,
        io: &impl Access,
        expected: &Device,
        target: Target,
        restore: bool,
    ) -> Result<()> {
        let current = find(io, &expected.id)?;
        if current != *expected {
            bail!("原厂独显状态或策略已变化，请重新读取并确认");
        }
        operation::preflight(&current, current.target(), target)?;
        if current.target() == target && !restore {
            return Ok(());
        }
        if let Err(error) = io
            .set(&current, target)
            .and_then(|_| verify(io, &current.id, target))
        {
            let restored = find(io, &current.id)
                .is_ok_and(|now| now.healthy() && now.target() == current.target());
            bail!(
                "原厂独显切换失败：{error:#}；{}",
                if restored {
                    "本次前态未变化或已补偿"
                } else {
                    "恢复未完成，请重新读取后重试"
                }
            );
        }
        Ok(())
    }
}
fn find(io: &impl Access, id: &str) -> Result<Device> {
    io.list()?
        .into_iter()
        .find(|d| d.id == id)
        .ok_or_else(|| anyhow!("原厂独显控制节点已移除，拒绝操作其他实例"))
}
fn verify(io: &impl Access, id: &str, target: Target) -> Result<()> {
    let current = find(io, id)?;
    if current.target() != target || !current.healthy() {
        bail!("原厂独显控制节点或策略回读不一致");
    }
    Ok(())
}
