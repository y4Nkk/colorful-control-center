use crate::hal::Hal;
use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
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
    pub initial: Option<Snapshot>,
    pub restore_revision: Option<String>,
    pub restore_error: Option<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub expected_revision: String,
    pub enabled: bool,
}
#[derive(Default)]
pub struct Controller {
    initial: Mutex<Option<Snapshot>>,
}
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
fn revision(current: &Snapshot, initial: &Option<Snapshot>) -> Result<String> {
    digest(&("airplane-state-1", current, initial))
}
fn profile(io: &dyn Io, initial: Option<Snapshot>) -> Result<Profile> {
    let current = io.current();
    let identity = current.as_ref().ok().map(Snapshot::identity).transpose()?;
    let revision = current
        .as_ref()
        .ok()
        .map(|s| revision(s, &initial))
        .transpose()?;
    let restore = initial
        .as_ref()
        .map(|first| {
            let now = current.as_ref().map_err(|error| anyhow!("{error:#}"))?;
            now.require_device(first)?;
            digest(&("restore-airplane-1", now, first))
        })
        .transpose();
    let (restore_revision, restore_error) = match restore {
        Ok(value) => (value, None),
        Err(error) => (None, Some(format!("{error:#}"))),
    };
    Ok(Profile {
        revision,
        identity,
        current: current.as_ref().ok().cloned(),
        error: current.err().map(|e| format!("{e:#}")),
        initial,
        restore_revision,
        restore_error,
    })
}
impl Controller {
    pub fn read(&self, hal: &Hal) -> Result<Profile> {
        let initial = self
            .initial
            .lock()
            .map_err(|_| anyhow!("飞行模式恢复状态异常"))?
            .clone();
        #[cfg(test)]
        {
            let _ = (hal, initial);
            bail!("隔离测试未注入飞行模式通道");
        }
        #[cfg(not(test))]
        {
            profile(&native::Native(hal), initial)
        }
    }
    pub fn apply(&self, hal: &Hal, request: &Request) -> Result<()> {
        self.change(hal, |io, initial| {
            let before = io.current()?;
            if revision(&before, initial)? != request.expected_revision {
                bail!("飞行模式预览已变化，请重新确认");
            }
            transaction::apply(io, initial, before, request.enabled, || Ok(()))
        })
    }
    pub(crate) fn toggle(
        &self,
        hal: &Hal,
        expected: &str,
        mut permit: impl FnMut() -> Result<()>,
    ) -> Result<bool> {
        self.change(hal, |io, initial| {
            permit()?;
            let before = io.current()?;
            if before.identity()? != expected {
                bail!("飞行模式接管来源已变化，请重新确认");
            }
            let wanted = !before.enabled;
            transaction::apply(io, initial, before, wanted, permit)?;
            Ok(wanted)
        })
    }
    pub fn restore(&self, hal: &Hal, expected: &str) -> Result<()> {
        self.change(hal, |io, initial| {
            let frame = profile(io, initial.clone())?;
            if frame.restore_revision.as_deref() != Some(expected) {
                bail!("飞行模式初始恢复预览已变化");
            }
            let before = frame.current.context("飞行模式当前状态未知")?;
            let wanted = initial.as_ref().context("飞行模式没有初始记录")?.enabled;
            transaction::apply(io, initial, before, wanted, || Ok(()))?;
            *initial = None;
            Ok(())
        })
    }
    fn change<T>(
        &self,
        hal: &Hal,
        change: impl FnOnce(&dyn Io, &mut Option<Snapshot>) -> Result<T>,
    ) -> Result<T> {
        let mut initial = self
            .initial
            .lock()
            .map_err(|_| anyhow!("飞行模式恢复状态异常"))?;
        #[cfg(test)]
        {
            let _ = (hal, change, &mut initial);
            bail!("隔离测试未注入飞行模式写入通道");
        }
        #[cfg(not(test))]
        {
            change(&native::Native(hal), &mut initial)
        }
    }
}
