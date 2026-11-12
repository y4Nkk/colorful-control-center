//! 性能调校域（Phase 3）：PL1/PL2 功耗墙 + 偏移电压（undervolt）。
//!
//! 双通道（契约 §10）：
//! - **持久化通道（默认，OEM 同款）**：AppSettings page6 写入 + IsSaved=1。
//!   与 OEM CPU_OC 完全同款语义；生效时机取决于 OEM 后台服务/固件
//!   （本机 XTU3SERVICE 已安装），失败时无副作用残留——值就是"意图"。
//! - **运行时通道（可选，需 PawnIO）**：MSR 0x610 直写 PL；0x150 OC Mailbox
//!   直写电压偏移（plane cpu/gpu/cache），写后回读校验（锁检测内建）。
//!
//! 钳位（安全设计 docs/design.md §4）：
//! - PL1/PL2：35–157 W（基准机默认 135/135，上限按散热保守取 157）
//! - 核心电压偏移：−150…+50 mV；GT 偏移：−150…0 mV

use anyhow::{anyhow, Context, Result};
use serde::Serialize;

use crate::hal::{pawnio, Hal};

pub const PL_MIN_W: f64 = 35.0;
pub const PL_MAX_W: f64 = 157.0;
pub const UV_MIN_MV: i32 = -150;
pub const UV_MAX_MV: i32 = 50;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PowerProfile {
    /// page6 当前值（固件意图）
    pub pl1_w: Option<f64>,
    pub pl2_w: Option<f64>,
    pub pl_time_s: Option<f64>,
    pub core_offset_mv: Option<f64>,
    pub gt_offset_mv: Option<f64>,
    /// WMI18 出厂默认（对照基准）
    pub default_pl1_w: Option<f64>,
    pub default_pl2_w: Option<f64>,
    pub default_core_offset_mv: Option<f64>,
    /// 运行时通道可用性（PawnIO）
    pub msr_runtime_available: bool,
    pub warnings: Vec<String>,
}

fn f32_le(buf: &[u8], at: usize) -> f32 {
    f32::from_le_bytes([buf[at], buf[at + 1], buf[at + 2], buf[at + 3]])
}
fn i32_le(buf: &[u8], at: usize) -> i32 {
    i32::from_le_bytes([buf[at], buf[at + 1], buf[at + 2], buf[at + 3]])
}
fn u16_le(buf: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([buf[at], buf[at + 1]])
}
fn u32_le(buf: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([buf[at], buf[at + 1], buf[at + 2], buf[at + 3]])
}

/// 读取当前性能配置（page6 + WMI18 默认 + 运行时通道可用性）。
pub fn read_profile(hal: &Hal) -> Result<PowerProfile> {
    let (page6, page18) = hal.with_dchu(|dchu| {
        Ok::<(Vec<u8>, [u8; 256]), anyhow::Error>((
            dchu.read_settings(6, 0, 256),
            dchu.read_package(18),
        ))
    })?;

    let scale_w = |raw: u32| -> f64 {
        if raw > 1_000_000 {
            raw as f64 / 1_000_000.0
        } else if raw > 1_000 {
            raw as f64 / 1_000.0
        } else {
            raw as f64
        }
    };

    let uv_raw = u16_le(&page18, 176);
    let default_uv = if (uv_raw >> 15) & 1 == 1 {
        (uv_raw ^ 0x8000) as i32
    } else {
        -(uv_raw as i32)
    };

    let msr_runtime_available = check_msr_runtime();

    Ok(PowerProfile {
        pl1_w: Some(i32_le(&page6, 33) as f64),
        pl2_w: Some(i32_le(&page6, 37) as f64),
        pl_time_s: Some(f32_le(&page6, 41) as f64),
        core_offset_mv: Some(i32_le(&page6, 61) as f64),
        gt_offset_mv: Some(i32_le(&page6, 69) as f64),
        default_pl1_w: Some(scale_w(u32_le(&page18, 8))),
        default_pl2_w: Some(scale_w(u32_le(&page18, 12))),
        default_core_offset_mv: Some(default_uv as f64),
        msr_runtime_available,
        warnings: Vec::new(),
    })
}

/// PawnIO 运行时通道探测（不执行写）：能打开设备即视为可用。
fn check_msr_runtime() -> bool {
    pawnio::PawnIo::open_with_intel_msr().is_ok()
}

fn clamp_pl(w: f64) -> Result<f64> {
    if !(PL_MIN_W..=PL_MAX_W).contains(&w) {
        return Err(anyhow!("功耗墙 {w}W 超出安全范围 {PL_MIN_W}–{PL_MAX_W}W"));
    }
    Ok(w)
}

fn clamp_uv(mv: i32, min: i32, max: i32, label: &str) -> Result<i32> {
    if !(min..=max).contains(&mv) {
        return Err(anyhow!("{label}偏移 {mv}mV 超出安全范围 {min}…{max}mV"));
    }
    Ok(mv)
}

/// 持久化通道：写 PL1/PL2（W，None=保持）→ IsSaved=1。
pub fn apply_pl_persist(hal: &Hal, pl1_w: Option<f64>, pl2_w: Option<f64>) -> Result<()> {
    let pl1 = pl1_w.map(clamp_pl).transpose()?;
    let pl2 = pl2_w.map(clamp_pl).transpose()?;
    hal.with_dchu(|dchu| {
        let page6 = dchu.read_settings(6, 0, 256);
        let mut patched = page6.clone();
        if let Some(v) = pl1 {
            patched[33..37].copy_from_slice(&(v as i32).to_le_bytes());
        }
        if let Some(v) = pl2 {
            patched[37..41].copy_from_slice(&(v as i32).to_le_bytes());
        }
        patched[32] = 1; // IsSaved：固件/服务应用 page6
        dchu.write_settings(6, 0, &patched)
    })
    .context("写入功耗墙（page6）失败")
}

/// 持久化通道：写电压偏移（mV，None=保持）→ IsSaved=1。
pub fn apply_voltage_persist(
    hal: &Hal,
    core_mv: Option<i32>,
    gt_mv: Option<i32>,
) -> Result<()> {
    let core = core_mv.map(|v| clamp_uv(v, UV_MIN_MV, UV_MAX_MV, "核心电压")).transpose()?;
    let gt = gt_mv.map(|v| clamp_uv(v, -150, 0, "GT 电压")).transpose()?;
    hal.with_dchu(|dchu| {
        let page6 = dchu.read_settings(6, 0, 256);
        let mut patched = page6.clone();
        if let Some(v) = core {
            patched[61..65].copy_from_slice(&v.to_le_bytes());
        }
        if let Some(v) = gt {
            patched[69..73].copy_from_slice(&v.to_le_bytes());
        }
        patched[32] = 1;
        dchu.write_settings(6, 0, &patched)
    })
    .context("写入电压偏移（page6）失败")
}

/// 运行时通道：MSR 0x610 直写 PL（读改写，保留时间窗与锁位）。
pub fn apply_pl_runtime(hal: &Hal, pl1_w: Option<f64>, pl2_w: Option<f64>) -> Result<()> {
    let pl1 = pl1_w.map(clamp_pl).transpose()?;
    let pl2 = pl2_w.map(clamp_pl).transpose()?;
    let io = pawnio::PawnIo::open_with_intel_msr()?;
    let current = io
        .read_msr(pawnio::MSR_PKG_POWER_LIMIT)
        .context("读取 MSR 0x610 失败")?;
    if (current >> 63) & 1 == 1 {
        return Err(anyhow!("MSR PL 锁定位(bit63)已置位：运行时直写不可用，请使用持久化通道"));
    }
    let value = pawnio::encode_power_limit(current, pl1, pl2);
    io.write_msr(pawnio::MSR_PKG_POWER_LIMIT, value)
        .context("写入 MSR 0x610 失败")?;
    let verify = io.read_msr(pawnio::MSR_PKG_POWER_LIMIT)?;
    if verify & 0x7FFF7FFF_00000000u64 & value != value & 0x7FFF7FFF_00000000u64 {
        // 粗校验：回读的 PL1/PL2 字段应与写入一致
        return Err(anyhow!("写入后回读不一致（可能被固件覆盖）"));
    }
    Ok(())
}

/// 运行时通道：OC Mailbox 写核心电压偏移（plane cpu=0 / gpu=1 / cache=2），
/// 写后回读；回读与目标偏差 >2mV 判定为锁定。
pub fn apply_voltage_runtime(
    hal: &Hal,
    plane: u64,
    mv: i32,
) -> Result<f64> {
    let _ = hal;
    let clamped = match plane {
        0 | 2 => clamp_uv(mv, UV_MIN_MV, UV_MAX_MV, "电压")?,
        1 => clamp_uv(mv, -150, 0, "GT 电压")?,
        _ => return Err(anyhow!("未知 plane：{plane}")),
    };
    let io = pawnio::PawnIo::open_with_intel_msr()?;
    let current = pawnio::read_voltage_offset(&io, plane).context("读取当前偏移失败")?;
    let after = pawnio::set_voltage_offset(&io, plane, clamped)?;
    if (after - clamped as f64).abs() > 2.0 {
        // 还原现场，避免半程状态
        let restored = pawnio::set_voltage_offset(&io, plane, current.round() as i32);
        let _ = restored;
        return Err(anyhow!("写入未生效（回读 {after:+.1}mV ≠ 目标 {clamped}mV）——OC Mailbox 疑似锁定"));
    }
    Ok(after)
}
