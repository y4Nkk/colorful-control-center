//! 原厂I2C触摸板开关；系统事实与OEM保存偏好分别建模。
use crate::hal::Hal;
use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::{Mutex, MutexGuard};

#[path = "touchpad/controller.rs"]
mod controller;
#[path = "touchpad/model.rs"]
mod model;
#[cfg(not(test))]
#[path = "touchpad/native.rs"]
mod native;
#[cfg(test)]
#[path = "touchpad/tests.rs"]
mod tests;
#[path = "touchpad/transaction.rs"]
mod transaction;
pub use controller::Controller;
pub(crate) use controller::Scope;
pub use model::{Profile, Request, Snapshot, Target};
pub(crate) use transaction::Plan;

pub(crate) trait Io {
    fn snapshot(&self) -> Result<Snapshot>;
    fn ensure_available(&self) -> Result<()>;
    fn set_enabled(&self, devices: &[String], before: bool, enabled: bool) -> Result<()>;
    fn save(&self, value: u8) -> Result<()>;
}
