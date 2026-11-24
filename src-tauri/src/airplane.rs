use crate::hal::Hal;
use anyhow::{Result, anyhow, bail};
use serde::{Deserialize, Serialize};
#[cfg(not(test))]
#[path = "airplane/native.rs"]
mod native;
#[cfg(test)]
#[path = "airplane/tests.rs"]
mod tests;
#[path = "airplane/transaction.rs"]
mod transaction;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub device: String,
    pub enabled: bool,
}
impl Snapshot {
    fn identity(&self) -> Result<String> {
        digest(&("oem-airplane-1", &self.device))
    }
    fn require_device(&self, expected: &Self) -> Result<()> {
        if self.device != expected.device {
            bail!("原厂飞行模式设备已变化");
        }
        Ok(())
    }
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub revision: Option<String>,
    pub identity: Option<String>,
    pub current: Option<Snapshot>,
    pub error: Option<String>,
    pub blocked_reason: Option<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub expected_revision: String,
    pub enabled: bool,
}
#[derive(Default)]
pub struct Controller;
trait Io {
    fn current(&self) -> Result<Snapshot>;
    fn require_device(&self, source: &Snapshot) -> Result<()>;
    fn ensure_writable(&self) -> Result<()>;
    fn firmware(&self, enabled: bool) -> Result<()>;
    fn system(&self, source: &Snapshot, enabled: bool) -> Result<()>;
    fn observed(
        &self,
        source: &Snapshot,
        enabled: bool,
        permit: &mut dyn FnMut() -> Result<()>,
    ) -> Result<()>;
}
fn digest(value: &impl Serialize) -> Result<String> {
    use sha2::{Digest, Sha256};
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(value)?)))
}
fn revision(current: &Snapshot) -> Result<String> {
    digest(&("airplane-state-1", current))
}
fn profile(io: &dyn Io) -> Result<Profile> {
    let current = io.current();
    let identity = current.as_ref().ok().map(Snapshot::identity).transpose()?;
    let revision = current.as_ref().ok().map(revision).transpose()?;
    let blocked_reason = if current.is_ok() {
        io.ensure_writable().err().map(|error| format!("{error:#}"))
    } else {
        None
    };
    Ok(Profile {
        revision,
        identity,
        current: current.as_ref().ok().cloned(),
        error: current.err().map(|e| format!("{e:#}")),
        blocked_reason,
    })
}
impl Controller {
    pub fn read(&self, hal: &Hal) -> Result<Profile> {
        #[cfg(test)]
        {
            let _ = hal;
            bail!("隔离测试未注入飞行模式通道");
        }
        #[cfg(not(test))]
        {
            profile(&native::Native(hal))
        }
    }
    pub fn apply(&self, hal: &Hal, request: &Request) -> Result<()> {
        self.change(hal, |io| {
            let before = io.current()?;
            if revision(&before)? != request.expected_revision {
                bail!("飞行模式预览已变化，请重新确认");
            }
            transaction::apply(io, before, request.enabled, || Ok(()))
        })
    }
    pub(crate) fn toggle(
        &self,
        hal: &Hal,
        expected: &str,
        mut permit: impl FnMut() -> Result<()>,
    ) -> Result<bool> {
        self.change(hal, |io| {
            permit()?;
            let before = io.current()?;
            if before.identity()? != expected {
                bail!("飞行模式接管来源已变化，请重新确认");
            }
            let wanted = !before.enabled;
            transaction::apply(io, before, wanted, permit)?;
            Ok(wanted)
        })
    }
    fn change<T>(
        &self,
        hal: &Hal,
        change: impl FnOnce(&dyn Io) -> Result<T>,
    ) -> Result<T> {
        #[cfg(test)]
        {
            let _ = (hal, change);
            bail!("隔离测试未注入飞行模式写入通道");
        }
        #[cfg(not(test))]
        {
            change(&native::Native(hal))
        }
    }
}
