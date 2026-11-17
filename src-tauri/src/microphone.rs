use crate::hal::Hal;
use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    sync::{Mutex, MutexGuard},
};

#[path = "microphone/controller.rs"]
mod controller;
#[path = "microphone/model.rs"]
mod model;
#[cfg(not(test))]
#[path = "microphone/native.rs"]
mod native;
#[cfg(test)]
#[path = "microphone/tests.rs"]
mod tests;
#[path = "microphone/transaction.rs"]
mod transaction;
pub use controller::Controller;
pub(crate) use controller::Scope;
pub use model::{Endpoint, Profile, Request, Snapshot, Target};
pub(crate) use transaction::Plan;

pub(crate) trait Io {
    fn snapshot(&self) -> Result<Snapshot>;
    fn ensure_available(&self) -> Result<()>;
    fn set_muted(&self, id: &str, muted: bool) -> Result<()>;
    fn save(&self, value: u8) -> Result<()>;
}
