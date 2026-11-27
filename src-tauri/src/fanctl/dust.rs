//! 原厂固件除尘；读取不修改缓存，手动控制的互斥检查也复用此协议。
mod native;
mod protocol;
mod scope;
#[cfg(test)]
mod tests;
mod transaction;
use crate::hal::Hal;
use anyhow::{Context, Result, anyhow, bail};
use ccc_core::settings::SettingsAccess;
pub use protocol::Schedule;
pub use scope::Controller;
pub(crate) use scope::{Plan, Scope};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

trait Io: SettingsAccess {
    fn running(&self) -> Result<Option<bool>>;
    fn fan_mode(&self) -> Result<u8>;
    fn duties(&self) -> Result<[f64; 2]>;
    fn sync_clock(&self) -> Result<()>;
    fn command(&self, sub: u8, value: u32) -> Result<()>;
}
#[derive(Serialize)]
struct Facts {
    saved: [u8; 5],
    running: bool,
}
fn field<const N: usize>(io: &impl SettingsAccess, page: u8, offset: usize) -> Result<[u8; N]> {
    io.read(page, offset, N)
        .map_err(|e| anyhow!(e))?
        .try_into()
        .map_err(|_| anyhow!("除尘字段长度无效"))
}
fn capture(io: &impl Io) -> Result<Option<Facts>> {
    let Some(running) = io.running()? else {
        return Ok(None);
    };
    let saved = field(io, 4, 81)?;
    protocol::decode(&saved)?;
    Ok(Some(Facts { saved, running }))
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
enum StartOutcome {
    Sent,
    Uncertain,
    Running,
    Completed,
}
#[derive(Default)]
struct State {
    baseline: Option<[u8; 5]>,
    starts: u64,
    last_start: Option<StartOutcome>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Initial {
    schedule: Option<Schedule>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    revision: String,
    running: bool,
    current: Option<Schedule>,
    initial: Option<Initial>,
    software_active: bool,
    last_start: Option<StartOutcome>,
    start_blocked_reason: Option<String>,
    schedule_blocked_reason: Option<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub expected_revision: String,
    pub schedule: Schedule,
}

fn start_gate(io: &impl Io, software: bool) -> Result<()> {
    if software {
        bail!("请先退出软件风扇曲线，再开始除尘");
    }
    let mode = io.fan_mode()?;
    if mode == super::FAN_MODE_FULL_SPEED {
        bail!("请先退出全速风扇模式");
    }
    super::validate_mode(mode)?;
    if io
        .duties()?
        .iter()
        .any(|v| !v.is_finite() || !(0.0..=50.0).contains(v))
    {
        bail!("CPU 和 GPU 风扇强度均不超过 50% 时才能开始除尘");
    }
    Ok(())
}
fn read(
    io: &impl Io,
    state: &mut State,
    software: bool,
    blocked: Option<String>,
) -> Result<Option<(Facts, Profile)>> {
    let Some(facts) = capture(io)? else {
        return Ok(None);
    };
    if facts.running && state.last_start.is_some() {
        state.last_start = Some(StartOutcome::Running);
    }
    if !facts.running && state.last_start == Some(StartOutcome::Running) {
        state.last_start = Some(StartOutcome::Completed);
    }
    let schedule_blocked_reason =
        blocked.or_else(|| facts.running.then(|| "固件正在除尘，请等待结束".into()));
    let start_blocked_reason = schedule_blocked_reason
        .clone()
        .or_else(|| start_gate(io, software).err().map(|e| format!("{e:#}")));
    let revision = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&(
            &facts,
            state.starts,
            state.last_start,
            software,
            &start_blocked_reason,
            &schedule_blocked_reason
        ))?)
    );
    let profile = Profile {
        revision,
        running: facts.running,
        current: protocol::decode(&facts.saved)?,
        initial: state
            .baseline
            .map(|raw| protocol::decode(&raw).map(|schedule| Initial { schedule }))
            .transpose()?,
        software_active: software,
        last_start: state.last_start,
        start_blocked_reason,
        schedule_blocked_reason,
    };
    Ok(Some((facts, profile)))
}
fn review(profile: &Profile, expected: &str, start: bool) -> Result<()> {
    if profile.revision != expected {
        bail!("除尘预览已变化，请重新读取并确认");
    }
    let blocked = if start {
        &profile.start_blocked_reason
    } else {
        &profile.schedule_blocked_reason
    };
    if let Some(reason) = blocked {
        bail!(reason.clone());
    }
    Ok(())
}
fn ensure_idle(io: &impl Io) -> Result<bool> {
    match io.running()? {
        Some(true) => bail!("固件正在除尘，请等待结束后再调整风扇或电源模式"),
        Some(false) => Ok(true),
        None => Ok(false),
    }
}
fn require_idle(io: &impl Io) -> Result<()> {
    ensure_idle(io)?.then_some(()).context("固件未声明除尘能力")
}
pub fn ensure_manual_allowed(hal: &Hal) -> Result<()> {
    hal.with_dchu(|io| ensure_idle(io).map(|_| ()))
}
pub fn ensure_software_allowed(hal: &Hal) -> Result<()> {
    hal.with_dchu(|io| {
        if ensure_idle(io)? && protocol::decode(&field(io, 4, 81)?)?.is_none_or(|s| s.enabled()) {
            bail!("请先明确关闭定时及唤醒除尘，再启用软件风扇曲线");
        }
        Ok(())
    })
}
fn start(io: &impl Io, state: &mut State, software: bool, expected: &str) -> Result<()> {
    let (_, profile) = read(io, state, software, None)?.context("固件未声明除尘能力")?;
    review(&profile, expected, true)?;
    require_idle(io)?;
    state.starts = state.starts.wrapping_add(1);
    state.last_start = Some(StartOutcome::Uncertain);
    io.command(41, 1)
        .context("除尘请求结果未确认，请重新读取状态后决定是否重试")?;
    state.last_start = Some(StartOutcome::Sent);
    Ok(())
}
fn apply(io: &impl Io, state: &mut State, software: bool, request: &Request) -> Result<()> {
    request.schedule.validate()?;
    let (before, profile) = read(io, state, software, None)?.context("固件未声明除尘能力")?;
    review(&profile, &request.expected_revision, false)?;
    if software && request.schedule.enabled() {
        bail!("软件风扇曲线运行时不能启用除尘计划");
    }
    transaction::apply(io, state, &before, request.schedule.saved())
}
fn restore(io: &impl Io, state: &mut State, software: bool, expected: &str) -> Result<()> {
    let (_, profile) = read(io, state, software, None)?.context("固件未声明除尘能力")?;
    review(&profile, expected, false)?;
    let raw = state.baseline.context("没有除尘计划初始快照")?;
    if software && protocol::decode(&raw)?.is_none_or(|s| s.enabled()) {
        bail!("请先退出软件风扇曲线，再恢复此除尘计划");
    }
    transaction::restore(io, state)
}
