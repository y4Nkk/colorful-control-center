//! PSTATES20 偏移事务（读 → 校验 → 写 → 回读 → 回滚）。
//!
//! 纯逻辑模块：经 `PstatesIo` 抽象 NVAPI IO，mock 可完整测试，不加载 DLL、
//! 不触碰 GPU。安全语义（docs/design.md §4）：
//! - 校验在任何写入之前：域存在、可编辑、整数 MHz 在有效区间
//!   （驱动区间 ∩ 上层传入的保守钳位）；
//! - 写入最小 V1 包（只携带请求域增量，未列出的字段驱动不动）；
//! - 回读确认：请求域回读值 ≠ 请求值即失败；
//! - 失败（含写入报错、回读失败）按**原始 kHz** 回滚并再次回读，
//!   回滚失败单独暴露，绝不吞掉。

use anyhow::{Context, Result, anyhow};

use super::pstates::{DomainReading, ParsedPstates};

/// 单域钳位（MHz）。
#[derive(Clone, Copy, Debug)]
pub struct DomainCaps {
    pub min_mhz: i32,
    pub max_mhz: i32,
}

/// 事务前后的原始读数（kHz；避免 MHz 往返取整漂移）。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RawOffsets {
    pub core_khz: Option<i32>,
    pub mem_khz: Option<i32>,
}

/// 事务结果。`ok=true` 时 confirmed* 为回读确认值；失败时 message 已含
/// 回滚说明，rollback_error 仅在回滚也失败时非空（硬件可能停在中间值）。
#[derive(Clone, Debug)]
pub struct TransactionOutcome {
    pub ok: bool,
    pub message: String,
    pub confirmed_core_mhz: Option<i32>,
    pub confirmed_mem_mhz: Option<i32>,
    pub rollback_error: Option<String>,
    /// 事务前的原始读数（成功时供上层记录恢复基准）
    pub pre: RawOffsets,
}

/// NVAPI 会话的最小 IO 面（真实实现见 hal::nvapi；测试用内存模拟）。
pub trait PstatesIo: Send + Sync {
    fn read(&self) -> Result<ParsedPstates>;
    /// 写入最小增量包（kHz；None 域不写）。
    fn write(&self, core_khz: Option<i32>, mem_khz: Option<i32>) -> Result<()>;
}

fn khz_to_mhz(khz: i32) -> i32 {
    khz / 1000
}

fn mhz_to_khz(mhz: i32) -> Result<i32> {
    mhz.checked_mul(1000)
        .ok_or_else(|| anyhow!("偏移 {mhz} MHz 超出可编码范围"))
}

/// 有效区间 = 驱动区间 ∩ 钳位；倒置区间不可写，不猜测缺失范围。
pub fn effective_range(reading: &DomainReading, caps: DomainCaps) -> (i32, i32) {
    if reading.min_khz > reading.max_khz {
        return (1, 0);
    }
    let driver_min = (reading.min_khz as f64 / 1000.0).ceil() as i32;
    let driver_max = (reading.max_khz as f64 / 1000.0).floor() as i32;
    (driver_min.max(caps.min_mhz), driver_max.min(caps.max_mhz))
}

fn raw_offsets(parsed: &ParsedPstates) -> RawOffsets {
    RawOffsets {
        core_khz: parsed.core.present.then_some(parsed.core.delta_khz),
        mem_khz: parsed.memory.present.then_some(parsed.memory.delta_khz),
    }
}

/// 校验请求（整数 MHz 在有效区间内、域存在且可编辑）→ 待写 (domain, kHz) 列表。
pub(crate) fn validate_requested(
    current: &ParsedPstates,
    core_mhz: Option<i32>,
    mem_mhz: Option<i32>,
    core_caps: DomainCaps,
    mem_caps: DomainCaps,
) -> std::result::Result<Vec<(u32, i32)>, String> {
    if !current.editable || !current.p0_present || current.count_clamped {
        return Err("GPU P0 表不可写或结构无效".into());
    }
    let mut requested = Vec::new();
    for (label, reading, caps, mhz, domain) in [
        ("核心", &current.core, core_caps, core_mhz, 0u32),
        ("显存", &current.memory, mem_caps, mem_mhz, 1),
    ] {
        let Some(mhz) = mhz else { continue };
        if !reading.present {
            return Err(format!("{label}时钟域不在 P0 表中，无法设置偏移"));
        }
        if !reading.editable {
            return Err(format!("驱动未开放{label}频率偏移调节（只读时钟）"));
        }
        let (min, max) = effective_range(reading, caps);
        if mhz < min || mhz > max {
            return Err(format!(
                "{label}偏移 {mhz:+} MHz 超出允许范围 {min:+}…{max:+} MHz"
            ));
        }
        // 区间校验已限 ±1000 MHz；checked_mul 为防御性兜底
        let khz = mhz_to_khz(mhz).map_err(|err| format!("{label}偏移非法：{err}"))?;
        requested.push((domain, khz));
    }
    Ok(requested)
}

/// 事务核心（纯逻辑）：读 → 校验 → 写 → 回读 →（失败）回滚。
pub fn run_transaction(
    io: &dyn PstatesIo,
    core_caps: DomainCaps,
    mem_caps: DomainCaps,
    core_mhz: Option<i32>,
    mem_mhz: Option<i32>,
) -> TransactionOutcome {
    let fail = |message: String| TransactionOutcome {
        ok: false,
        message,
        confirmed_core_mhz: None,
        confirmed_mem_mhz: None,
        rollback_error: None,
        pre: RawOffsets::default(),
    };
    if core_mhz.is_none() && mem_mhz.is_none() {
        return fail("未提供任何偏移（核心与显存均为空）".into());
    }
    let current = match io.read() {
        Ok(parsed) => parsed,
        Err(err) => return fail(format!("读取当前偏移失败：{err:#}")),
    };
    let requested = match validate_requested(&current, core_mhz, mem_mhz, core_caps, mem_caps) {
        Ok(list) => list,
        Err(message) => return fail(message),
    };
    let mut pre = raw_offsets(&current);
    if core_mhz.is_none() {
        pre.core_khz = None;
    }
    if mem_mhz.is_none() {
        pre.mem_khz = None;
    }

    match write_and_verify(io, &requested) {
        Ok(after) => {
            let fmt = |r: &DomainReading| {
                if r.present {
                    format!("{:+}", khz_to_mhz(r.delta_khz))
                } else {
                    "未上报".into()
                }
            };
            TransactionOutcome {
                ok: true,
                message: format!(
                    "GPU 偏移已写入并回读确认：核心 {} MHz / 显存 {} MHz",
                    fmt(&after.core),
                    fmt(&after.memory)
                ),
                confirmed_core_mhz: after
                    .core
                    .present
                    .then_some(khz_to_mhz(after.core.delta_khz)),
                confirmed_mem_mhz: after
                    .memory
                    .present
                    .then_some(khz_to_mhz(after.memory.delta_khz)),
                rollback_error: None,
                pre,
            }
        }
        Err(failure) => TransactionOutcome {
            ok: false,
            message: format!("{failure}；已尝试回滚到写入前状态"),
            confirmed_core_mhz: None,
            confirmed_mem_mhz: None,
            rollback_error: rollback_to(io, &pre),
            pre,
        },
    }
}

/// 写入请求域并回读；请求域回读值必须精确匹配。
pub(crate) fn write_and_verify(
    io: &dyn PstatesIo,
    requested: &[(u32, i32)],
) -> Result<ParsedPstates> {
    let core_khz = requested.iter().find(|(d, _)| *d == 0).map(|(_, v)| *v);
    let mem_khz = requested.iter().find(|(d, _)| *d == 1).map(|(_, v)| *v);
    io.write(core_khz, mem_khz)
        .context("写入 GPU 频率偏移失败")?;
    let after = io.read().context("写入后回读失败")?;
    for (domain, want) in requested {
        let reading = if *domain == 0 {
            &after.core
        } else {
            &after.memory
        };
        if !reading.present || !after.p0_present || reading.delta_khz != *want {
            return Err(anyhow!(
                "写入未生效：{}回读 {:+} MHz ≠ 请求 {:+} MHz",
                if *domain == 0 { "核心" } else { "显存" },
                khz_to_mhz(reading.delta_khz),
                khz_to_mhz(*want)
            ));
        }
    }
    Ok(after)
}

/// 按原始 kHz 回滚并回读验证；None=回滚成功，Some=回滚失败的完整错误。
pub fn restore_raw(io: &dyn PstatesIo, pre: &RawOffsets) -> Result<()> {
    io.write(pre.core_khz, pre.mem_khz)
        .context("恢复写入失败")?;
    let after = io.read().context("恢复后回读失败")?;
    for (label, want, got) in [
        ("核心", pre.core_khz, &after.core),
        ("显存", pre.mem_khz, &after.memory),
    ] {
        if let Some(want) = want {
            if !after.p0_present || !got.present || got.delta_khz != want {
                return Err(anyhow!(
                    "恢复{label}失败：期望 {want}kHz，实际 {}kHz",
                    got.delta_khz
                ));
            }
        }
    }
    Ok(())
}

fn rollback_to(io: &dyn PstatesIo, pre: &RawOffsets) -> Option<String> {
    let result = (|| -> Result<()> {
        io.write(pre.core_khz, pre.mem_khz)
            .context("回滚写入失败")?;
        let after = io.read().context("回滚后回读失败")?;
        for (label, want, got) in [
            ("核心", pre.core_khz, after.core.delta_khz),
            ("显存", pre.mem_khz, after.memory.delta_khz),
        ] {
            if let Some(want) = want {
                if got != want {
                    return Err(anyhow!(
                        "回滚后{label}回读 {:+} MHz ≠ {:+} MHz",
                        khz_to_mhz(got),
                        khz_to_mhz(want)
                    ));
                }
            }
        }
        Ok(())
    })();
    match result {
        Ok(()) => None,
        Err(err) => Some(format!(
            "回滚失败：{err:#}——GPU 偏移可能停留在中间值，请在 NVIDIA 控制面板核对或重启驱动"
        )),
    }
}

// ---------------------------------------------------------------------------
// 单元测试：全部走内存 mock，不加载 DLL、不触碰 GPU。
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "transaction/tests.rs"]
mod tests;
