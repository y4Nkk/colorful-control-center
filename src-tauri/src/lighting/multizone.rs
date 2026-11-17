//! Type23 uses cmd4/sub44, independently of ordinary cmd103 and per-key HID.
use super::protocol::Rgb;
use crate::hal::Hal;
use anyhow::{Context, Result, anyhow, bail};
use ccc_core::settings::Patch;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
#[path = "multizone/hotkey.rs"]
mod hotkey;
#[path = "multizone/model.rs"]
mod model;
#[path = "multizone/native.rs"]
mod native;
#[path = "multizone/scope.rs"]
mod scope;
#[cfg(test)]
#[path = "multizone/tests.rs"]
mod tests;
pub(crate) use model::Snapshot;
pub use model::{Capabilities, Profile, Request, Target, Values};
pub use scope::Controller;
pub(crate) use scope::{Plan, Scope};

pub(crate) trait Io {
    fn snapshot(&self) -> Result<Snapshot>;
    fn ensure_available(&self) -> Result<()>;
    fn send(&self, packet: &[u8; 37]) -> Result<()>;
    fn save(&self, patches: &[Patch]) -> Result<()>;
}
fn restore(io: &dyn Io, before: &Snapshot) -> Result<()> {
    let current = io.snapshot()?;
    current.require_device(before)?;
    let desired = current.target(&Target {
        capabilities: before.capabilities.clone(),
        values: before.values()?,
    })?;
    let mut errors = Vec::new();
    if let Err(error) = io.send(&desired.packet()?) {
        errors.push(format!("多区即时恢复：{error:#}"));
    }
    if let Err(error) = io.save(&desired.patches(&current)?) {
        errors.push(format!("多区保存恢复：{error:#}"));
    }
    if !errors.is_empty() {
        bail!(errors.join("；"));
    }
    Ok(())
}
fn apply(
    io: &dyn Io,
    baseline: &mut Option<Snapshot>,
    request: &Request,
    dirty: &mut bool,
) -> Result<()> {
    io.ensure_available()?;
    let before = io.snapshot()?;
    before.values()?;
    if before.revision() != request.expected_revision {
        bail!("多区键盘预览已变化，请重新读取后确认");
    }
    if let Some(initial) = baseline.as_ref() {
        before.require_device(initial)?;
    }
    let target = before.target(&request.target)?;
    let patches = target.patches(&before)?;
    if patches.is_empty() {
        return Ok(());
    }
    baseline.get_or_insert_with(|| before.clone());
    *dirty = true;
    let result = (|| -> Result<()> {
        io.send(&target.packet()?)?;
        io.save(&patches)?;
        let after = io.snapshot()?;
        after.require_device(&target)?;
        if after.revision() != target.revision() {
            bail!("多区键盘保存回读不一致");
        }
        Ok(())
    })();
    if let Err(error) = result {
        let recovery = restore(io, &before).map_err(|error| format!("{error:#}"));
        if recovery.is_ok() {
            *dirty = false;
        }
        bail!(
            "{}",
            ccc_core::msr::with_recovery(format!("多区键盘写入失败：{error:#}"), recovery)
        );
    }
    Ok(())
}
