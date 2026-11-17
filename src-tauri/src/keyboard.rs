//! FnKey/CC3 的共享键盘设置包；原始第4/5项不由本模块编辑。
use crate::hal::{Hal, oem_dchu::Dchu};
use anyhow::{Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Mutex;
#[path = "keyboard/scope.rs"]
mod scope;
pub(crate) use scope::{Plan, Scope};
#[cfg(test)]
#[path = "keyboard/tests.rs"]
mod tests;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Snapshot {
    live: [u8; 5],
    saved: [u8; 3],
}
impl Snapshot {
    fn revision(&self) -> String {
        format!(
            "{:x}",
            Sha256::digest([self.live.as_slice(), self.saved.as_slice()].concat())
        )
    }
    fn values(&self) -> Values {
        Values {
            fn_locked: flag(self.live[0], 1),
            win_locked: flag(self.live[1], 2),
            fn_win_swap: flag(self.live[2], 1),
        }
    }
}
fn flag(raw: u8, enabled: u8) -> Option<bool> {
    (raw != 0).then_some(raw == enabled)
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Values {
    pub fn_locked: Option<bool>,
    pub win_locked: Option<bool>,
    pub fn_win_swap: Option<bool>,
}
impl Values {
    pub fn is_empty(&self) -> bool {
        self.fn_locked.is_none() && self.win_locked.is_none() && self.fn_win_swap.is_none()
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub expected_revision: String,
    pub values: Values,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub revision: String,
    pub current: Values,
    pub can_restore: bool,
}
trait Io {
    fn live(&self) -> Result<[u8; 5]>;
    fn saved(&self) -> Result<[u8; 3]>;
    fn send(&self, values: [u8; 5]) -> Result<()>;
    fn save(&self, values: [u8; 3]) -> Result<()>;
}
impl Io for Dchu {
    fn live(&self) -> Result<[u8; 5]> {
        let mut packet = [0; 256];
        packet[0] = 24;
        let result = self.exchange_package(4, &packet)?;
        Ok(result[..5].try_into().expect("固定长度包"))
    }
    fn saved(&self) -> Result<[u8; 3]> {
        self.read_settings(1, 36, 3)?
            .try_into()
            .map_err(|_| anyhow!("键盘保存值长度错误"))
    }
    fn send(&self, values: [u8; 5]) -> Result<()> {
        let mut packet = [0; 256];
        packet[0] = 25;
        packet[1..6].copy_from_slice(&values);
        self.exchange_package(4, &packet)?;
        Ok(())
    }
    fn save(&self, values: [u8; 3]) -> Result<()> {
        self.write_settings(1, 36, &values)
    }
}
fn read(io: &impl Io) -> Result<Snapshot> {
    let result = Snapshot {
        live: io.live()?,
        saved: io.saved()?,
    };
    if result.live[..3].iter().any(|v| *v > 2) {
        bail!("未知键盘状态，未开放写入");
    }
    Ok(result)
}
fn write(io: &impl Io, target: &Snapshot) -> Result<()> {
    io.send(target.live)?;
    if io.live()? != target.live {
        bail!("键盘即时状态回读不一致");
    }
    io.save(target.saved)?;
    if io.saved()? != target.saved {
        bail!("键盘保存值回读不一致");
    }
    Ok(())
}
fn restore(io: &impl Io, original: &Snapshot) -> Result<()> {
    let current = read(io)?;
    require_same_capabilities(&current, original)?;
    let mut target = original.clone();
    target.live[3..].copy_from_slice(&current.live[3..]);
    let mut errors = Vec::new();
    if let Err(error) = io.send(target.live).and_then(|_| {
        if io.live()? != target.live {
            bail!("键盘恢复即时状态回读不一致");
        }
        Ok(())
    }) {
        errors.push(format!("即时恢复：{error:#}"));
    }
    if let Err(error) = io.save(target.saved).and_then(|_| {
        if io.saved()? != target.saved {
            bail!("键盘恢复保存值回读不一致");
        }
        Ok(())
    }) {
        errors.push(format!("保存恢复：{error:#}"));
    }
    if !errors.is_empty() {
        bail!(errors.join("；"));
    }
    Ok(())
}
fn require_same_capabilities(current: &Snapshot, original: &Snapshot) -> Result<()> {
    if (0..3).any(|i| (current.live[i] == 0) != (original.live[i] == 0)) {
        bail!("键盘支持能力已变化，拒绝写入旧快照");
    }
    Ok(())
}
fn target(before: &Snapshot, values: &Values) -> Result<Snapshot> {
    let mut result = before.clone();
    for (i, desired) in [values.fn_locked, values.win_locked, values.fn_win_swap]
        .into_iter()
        .enumerate()
    {
        if (before.live[i] == 0) != desired.is_none() {
            bail!("只能修改固件已声明支持的键盘功能");
        }
        if let Some(enabled) = desired {
            result.live[i] = if enabled == (i != 1) { 1 } else { 2 };
            if result.live[i] != before.live[i] {
                result.saved[i] = result.live[i];
            }
        }
    }
    Ok(result)
}
fn apply(io: &impl Io, baseline: &mut Option<Snapshot>, request: &Request) -> Result<()> {
    apply_tracked(io, baseline, request, &mut false)
}
fn apply_tracked(
    io: &impl Io,
    baseline: &mut Option<Snapshot>,
    request: &Request,
    needs_rollback: &mut bool,
) -> Result<()> {
    let before = read(io)?;
    if before.revision() != request.expected_revision {
        bail!("键盘状态或保存值已变化，请重新读取并确认");
    }
    if let Some(original) = baseline.as_ref() {
        require_same_capabilities(&before, original)?;
    }
    let next = target(&before, &request.values)?;
    if next == before {
        return Ok(());
    }
    baseline.get_or_insert_with(|| before.clone());
    *needs_rollback = true;
    if let Err(error) = write(io, &next) {
        let recovery = restore(io, &before).map_err(|e| format!("{e:#}"));
        if recovery.is_ok() {
            *needs_rollback = false;
        }
        bail!(ccc_core::msr::with_recovery(
            format!("键盘设置失败：{error:#}"),
            recovery
        ));
    }
    Ok(())
}
#[derive(Default)]
pub struct Controller {
    baseline: Mutex<Option<Snapshot>>,
}
impl Controller {
    pub fn read(&self, hal: &Hal) -> Result<Profile> {
        let scope = self.scope()?;
        let _gate = hal.begin_write()?;
        hal.with_dchu(|io| {
            let snapshot = read(io)?;
            Ok(Profile {
                revision: snapshot.revision(),
                current: snapshot.values(),
                can_restore: scope.baseline.is_some(),
            })
        })
    }
    pub fn apply(&self, hal: &Hal, request: &Request) -> Result<()> {
        let mut scope = self.scope()?;
        let _write = hal.begin_write()?;
        hal.with_dchu(|io| apply(io, &mut scope.baseline, request))
    }
    pub fn restore(&self, hal: &Hal) -> Result<()> {
        let scope = self.scope()?;
        let _write = hal.begin_write()?;
        let original = scope
            .baseline
            .as_ref()
            .ok_or_else(|| anyhow!("没有键盘初始快照"))?;
        hal.with_dchu(|io| restore(io, original))
    }
}
