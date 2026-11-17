//! LAN驱动策略、固定字段、授权会话和持久恢复的唯一所有者。
use crate::hal::Hal;
use anyhow::{Context, Result, anyhow, bail};
use model::{Change, Frame, Request};
use serde::Serialize;
use std::{path::PathBuf, sync::Mutex};
use store::Record;
#[path = "lan/authorization.rs"]
pub(crate) mod authorization;
#[path = "lan/environment.rs"]
mod environment;
pub(crate) use environment::Environment;
#[path = "lan/model.rs"]
pub mod model;
#[path = "../hal/network_policy.rs"]
pub mod native;
#[path = "lan/recovery.rs"]
mod recovery;
#[path = "lan/worker.rs"]
pub(crate) mod worker;
pub use worker::run_child;
#[path = "lan/store.rs"]
pub mod store;
#[path = "lan/target.rs"]
mod target;
#[path = "lan/transaction.rs"]
pub mod transaction;
pub use target::Target;
#[path = "lan/scope.rs"]
mod scope;
pub(crate) use scope::{Plan, Scope};
pub const RECORD_NAME: &str = "lan-recovery.json";
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub revision: String,
    pub frame: Frame,
    pub initial: Vec<store::Initial>,
    pub last_error: Option<String>,
    pub blocked_reason: Option<String>,
}
pub struct Controller {
    path: PathBuf,
    mutation: Mutex<()>,
    #[cfg(test)]
    test_io: Option<std::sync::Arc<dyn Environment>>,
}
impl Controller {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            mutation: Mutex::new(()),
            #[cfg(test)]
            test_io: None,
        }
    }
    #[cfg(test)]
    pub(crate) fn with_test(path: PathBuf, io: std::sync::Arc<dyn Environment>) -> Self {
        let mut controller = Self::new(path);
        controller.test_io = Some(io);
        controller
    }
    fn environment(&self) -> Result<&dyn Environment> {
        #[cfg(test)]
        return self
            .test_io
            .as_deref()
            .ok_or_else(|| anyhow!("隔离LAN控制器未注入环境，未访问真实设备或授权"));
        #[cfg(not(test))]
        Ok(&environment::Native)
    }
    pub fn read(&self, hal: &Hal) -> Result<Option<Profile>> {
        let scope = self.scope()?;
        let _gate = hal.begin_write()?;
        scope.read(hal)
    }
    pub fn preview(&self, hal: &Hal, request: &Request) -> Result<Vec<Change>> {
        let scope = self.scope()?;
        let _gate = hal.begin_write()?;
        scope.preview(hal, request)
    }
    pub fn apply(&self, hal: &Hal, request: &Request) -> Result<()> {
        self.execute(hal, request, false)
    }
    pub fn restore(&self, hal: &Hal, expected: &str, device: &str) -> Result<()> {
        self.execute(
            hal,
            &Request {
                expected_revision: expected.into(),
                device_id: device.into(),
                power_saving: None,
                apply_wake_patch: false,
            },
            true,
        )
    }
    fn execute(&self, hal: &Hal, request: &Request, restoring: bool) -> Result<()> {
        let mut scope = self.scope()?;
        let plan = {
            let _gate = hal.begin_write()?;
            scope.prepare_request(hal, request, restoring)?
        };
        let Some(plan) = plan else {
            return Ok(());
        };
        scope.authorize(&plan)?;
        let result = {
            let _gate = hal.begin_write()?;
            let mut touched = false;
            let result = scope.execute(hal, &plan, &mut touched, false);
            if !restoring && result.is_err() && touched {
                let recovery = scope.rollback(hal, &plan, &mut touched, false);
                scope::combine(result, recovery, "LAN本次补偿")
            } else {
                result
            }
        };
        scope::combine(result, scope.finish(), "LAN授权收尾")
    }
}
#[cfg(test)]
#[path = "lan/test_fixture.rs"]
pub(crate) mod test_fixture;
#[cfg(test)]
#[path = "lan/tests.rs"]
mod tests;
