//! X170/X58独立灯条：共享19B保存块，严格区分两套64B HID协议。
use super::protocol::Rgb;
use crate::hal::Hal;
use anyhow::{Context, Result, anyhow, bail};
use sha2::{Digest, Sha256};
#[cfg(not(test))]
#[path = "../hal/line_led.rs"]
mod hid;
#[path = "lightbar/model.rs"]
mod model;
#[cfg(not(test))]
#[path = "lightbar/native.rs"]
mod native;
#[path = "lightbar/scope.rs"]
mod scope;
#[cfg(test)]
#[path = "lightbar/tests.rs"]
mod tests;
#[path = "lightbar/x58.rs"]
mod x58;
#[cfg(test)]
pub(crate) use model::Kind;
pub(crate) use model::Snapshot;
pub use model::{Profile, Request, Values};
pub use scope::Controller;
pub(crate) use scope::{Plan, Scope};

pub(crate) trait Io {
    fn snapshot(&self) -> Result<Snapshot>;
    fn ensure_available(&self) -> Result<()>;
    fn send(&self, packet: &[u8; 64]) -> Result<()>;
    fn save(&self, bytes: &[u8; 19]) -> Result<()>;
}
fn restore(io: &(impl Io + ?Sized), snapshot: &Snapshot) -> Result<()> {
    let mut errors = Vec::new();
    for packet in snapshot.packets()? {
        if let Err(error) = io.send(&packet) {
            errors.push(format!("灯条即时恢复：{error:#}"));
        }
    }
    if let Err(error) = io.save(&snapshot.bytes) {
        errors.push(format!("灯条保存恢复：{error:#}"));
    }
    if !errors.is_empty() {
        bail!(errors.join("；"));
    }
    Ok(())
}
fn apply_tracked(
    io: &(impl Io + ?Sized),
    baseline: &mut Option<Snapshot>,
    request: &Request,
    needs_rollback: &mut bool,
) -> Result<()> {
    io.ensure_available()?;
    let before = io.snapshot()?;
    if before.revision() != request.expected_revision {
        bail!("灯条预览已变化，请重新读取确认");
    }
    if baseline.as_ref().is_some_and(|s| !s.same_device(&before)) {
        bail!("灯条设备已变化，初始恢复记录已保留");
    }
    let target = before.target(&request.values)?;
    if target.bytes == before.bytes {
        return Ok(());
    }
    baseline.get_or_insert_with(|| before.clone());
    *needs_rollback = true;
    let result = (|| -> Result<()> {
        for packet in target.packets()? {
            io.send(&packet)?;
        }
        io.save(&target.bytes)
    })();
    if let Err(error) = result {
        let recovery = restore(io, &before).map_err(|e| format!("{e:#}"));
        if recovery.is_ok() {
            *needs_rollback = false;
        }
        bail!(
            "{}",
            ccc_core::msr::with_recovery(format!("灯条写入失败：{error:#}"), recovery)
        );
    }
    Ok(())
}
fn restore_initial(
    io: &(impl Io + ?Sized),
    baseline: &Option<Snapshot>,
    expected: &str,
) -> Result<()> {
    io.ensure_available()?;
    let initial = baseline.as_ref().context("没有灯条初始快照")?;
    let current = io.snapshot()?;
    if current.revision() != expected || !current.same_device(initial) {
        bail!("灯条状态或设备变化，请重新预览恢复");
    }
    restore(io, initial)
}
fn profile(
    current: &Snapshot,
    baseline: &Option<Snapshot>,
    blocked: Option<String>,
) -> Result<Profile> {
    let values = current.values();
    let mut warnings = vec!["显示原厂保存配置；HID发送结果不代表物理灯效回读。".into()];
    if let Err(error) = &values {
        warnings.push(format!("{error:#}"));
    }
    Ok(Profile {
        kind: current.kind,
        revision: current.revision(),
        presets: if values.is_ok() {
            current.presets()
        } else {
            vec![]
        },
        current: values.ok(),
        initial: baseline.as_ref().map(|s| s.values()).transpose()?,
        warnings,
        blocked_reason: blocked.or_else(|| {
            baseline
                .as_ref()
                .filter(|s| !s.same_device(current))
                .map(|_| "灯条设备已变化，初始快照已保留，请重新连接原设备后恢复。".into())
        }),
    })
}
