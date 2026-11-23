//! GPU 频率偏移域（Phase 3）：NVAPI 核心/显存时钟偏移。
//!
//! 控制器职责：惰性 NVAPI 会话（Mutex 串行）、有效区间装配（驱动 ∩ 保守钳位：
//! 核心 ±300 / 显存 ±1000 MHz）、读侧事实（GpuProfile）、事务委托与结果映射
//! （transaction::run_transaction）、原始基准的记录与恢复（首次成功写入前记录，
//! 之后不被覆盖）。事务语义（校验→最小写入→回读→回滚）见 hal/nvapi/transaction.rs。
//!
//! `new()` 与单元测试不触碰硬件；只有运行时命令才建立 NVAPI 会话。

use std::sync::{Arc, Mutex};

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};

#[cfg(not(test))]
use crate::hal::nvapi::NvApiSession;
use crate::hal::nvapi::pstates::ParsedPstates;
use crate::hal::nvapi::transaction::{self, DomainCaps, PstatesIo, RawOffsets, TransactionOutcome};
use crate::hal::nvapi::turbo::{DeviceId, TurboIo};

#[cfg(test)]
#[path = "gpuctl/test_identity.rs"]
mod test_identity;
#[path = "gpuctl/turbo.rs"]
pub(crate) mod turbo;

/// 保守应用钳位（核心 ±300 MHz）。
pub const CORE_CAP_MHZ: (i32, i32) = (-300, 300);
/// 保守应用钳位（显存 ±1000 MHz）。
pub const MEM_CAP_MHZ: (i32, i32) = (-1000, 1000);

/// 档案可序列化的偏移设置（camelCase；供 profiles 域捕获/应用）。
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GpuClockSettings {
    pub core_offset_mhz: Option<i32>,
    pub mem_offset_mhz: Option<i32>,
}

/// 同时绑定物理设备身份与原始 kHz，供档案和首次恢复共用。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GpuCheckpoint {
    pub device: DeviceId,
    pub core_khz: Option<i32>,
    pub mem_khz: Option<i32>,
}
impl GpuCheckpoint {
    fn new(device: DeviceId, values: RawOffsets) -> Self {
        Self {
            device,
            core_khz: values.core_khz,
            mem_khz: values.mem_khz,
        }
    }
    fn raw(self) -> RawOffsets {
        RawOffsets {
            core_khz: self.core_khz,
            mem_khz: self.mem_khz,
        }
    }
}

/// 读侧事实（camelCase，read_gpu_profile 返回体）。
#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct GpuProfile {
    /// NVAPI 会话 + GPU 枚举 + P-States 读取全链路可用
    pub available: bool,
    /// 不可用时的确切原因（阶段 + NVAPI 状态）
    pub unavailable_reason: Option<String>,
    /// 单项调校 IPC 的写入权限受限原因，由命令层填充。
    pub write_block: Option<GpuWriteBlock>,
    pub gpu_name: Option<String>,
    pub core_offset_mhz: Option<i32>,
    pub mem_offset_mhz: Option<i32>,
    pub core_editable: bool,
    pub mem_editable: bool,
    /// 有效区间（驱动 ∩ 钳位）；驱动未报告区间时为钳位本身
    pub core_min_mhz: i32,
    pub core_max_mhz: i32,
    pub mem_min_mhz: i32,
    pub mem_max_mhz: i32,
    /// 应用自有写入前的原始基准（存在即可恢复）
    pub baseline: Option<GpuClockSettings>,
    pub warnings: Vec<String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GpuWriteBlock {
    pub reason: String,
    pub can_request_elevation: bool,
}

/// 写侧事务结果（camelCase）。
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ApplyOutcome {
    pub ok: bool,
    pub message: String,
    /// 回读确认的实际偏移（仅成功时；域未上报为 null）
    pub core_offset_mhz: Option<i32>,
    pub mem_offset_mhz: Option<i32>,
    /// 回滚亦失败时的完整错误（硬件可能处于中间态，必须暴露）
    pub rollback_error: Option<String>,
}

impl ApplyOutcome {
    fn failure(message: String) -> Self {
        Self {
            ok: false,
            message,
            core_offset_mhz: None,
            mem_offset_mhz: None,
            rollback_error: None,
        }
    }

    fn from_transaction(outcome: TransactionOutcome) -> Self {
        Self {
            ok: outcome.ok,
            message: outcome.message,
            core_offset_mhz: outcome.confirmed_core_mhz,
            mem_offset_mhz: outcome.confirmed_mem_mhz,
            rollback_error: outcome.rollback_error,
        }
    }
}

#[derive(Default)]
struct ControllerState {
    session: Option<Arc<dyn PstatesIo>>,
    turbo_io: Option<Arc<dyn TurboIo>>,
    turbo: turbo::State,
    gpu_name: Option<String>,
    baseline: Option<GpuCheckpoint>,
    power_suspended: bool,
    #[cfg(test)]
    test_source: Option<Arc<dyn PstatesIo>>,
    #[cfg(test)]
    test_turbo: Option<Arc<dyn TurboIo>>,
}

/// GPU 偏移控制器（tauri manage 单例；构造不触碰硬件）。
pub struct GpuController {
    state: Mutex<ControllerState>,
}

impl Default for GpuController {
    fn default() -> Self {
        Self::new()
    }
}

impl GpuController {
    #[cfg(test)]
    pub(crate) fn test_with_session(io: Arc<dyn PstatesIo>) -> Self {
        Self::test_with_sources(io, Arc::new(test_identity::Identity::default()))
    }
    #[cfg(test)]
    pub(crate) fn test_with_sources(io: Arc<dyn PstatesIo>, turbo: Arc<dyn TurboIo>) -> Self {
        Self {
            state: Mutex::new(ControllerState {
                session: Some(io.clone()),
                test_source: Some(io),
                turbo_io: Some(turbo.clone()),
                test_turbo: Some(turbo),
                ..Default::default()
            }),
        }
    }

    pub fn new() -> Self {
        Self {
            state: Mutex::new(ControllerState::default()),
        }
    }

    /// 原厂电源切换期间释放连接，关断后禁止读侧重连，保留首次偏移基线。
    pub(crate) fn set_power_suspended(&self, suspended: bool) -> Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow!("GPU 偏移连接状态异常"))?;
        if state.power_suspended == suspended {
            return Ok(());
        }
        disconnect_session(&mut state);
        state.power_suspended = suspended;
        Ok(())
    }

    /// 读取当前偏移事实。永不返回 Err——可用性折叠进 `available/reason`。
    pub fn read_profile(&self) -> GpuProfile {
        let mut state = self.state.lock().unwrap();
        match ensure_session(&mut state).and_then(|io| {
            let device = turbo::current_device(&mut state)?;
            Ok((io, device))
        }) {
            Err(err) => GpuProfile {
                unavailable_reason: Some(err.to_string()),
                core_min_mhz: CORE_CAP_MHZ.0,
                core_max_mhz: CORE_CAP_MHZ.1,
                mem_min_mhz: MEM_CAP_MHZ.0,
                mem_max_mhz: MEM_CAP_MHZ.1,
                ..Default::default()
            },
            Ok((io, device)) => match io.read() {
                Ok(parsed) => profile_from(&parsed, state.gpu_name.as_deref(), &state, device),
                Err(err) => GpuProfile {
                    unavailable_reason: Some(err.to_string()),
                    core_min_mhz: CORE_CAP_MHZ.0,
                    core_max_mhz: CORE_CAP_MHZ.1,
                    mem_min_mhz: MEM_CAP_MHZ.0,
                    mem_max_mhz: MEM_CAP_MHZ.1,
                    ..Default::default()
                },
            },
        }
    }

    /// 事务式应用偏移（None = 该域不写）。成功且尚无基准时记录原始偏移。
    pub fn apply_offsets(&self, core_mhz: Option<i32>, mem_mhz: Option<i32>) -> ApplyOutcome {
        let mut state = self.state.lock().unwrap();
        let io = match ensure_session(&mut state) {
            Ok(io) => io,
            Err(err) => return ApplyOutcome::failure(err.to_string()),
        };
        let device = match turbo::current_device(&mut state) {
            Ok(id) => id,
            Err(error) => return ApplyOutcome::failure(format!("{error:#}")),
        };
        if state
            .baseline
            .is_some_and(|baseline| baseline.device != device)
        {
            return ApplyOutcome::failure("GPU 身份已变化，不能复用其他设备的初始偏移".into());
        }
        let outcome =
            transaction::run_transaction(io.as_ref(), core_caps(), mem_caps(), core_mhz, mem_mhz);
        if outcome.ok || outcome.rollback_error.is_some() {
            let baseline = state
                .baseline
                .get_or_insert_with(|| GpuCheckpoint::new(device, RawOffsets::default()));
            if core_mhz.is_some() && baseline.core_khz.is_none() {
                baseline.core_khz = outcome.pre.core_khz;
            }
            if mem_mhz.is_some() && baseline.mem_khz.is_none() {
                baseline.mem_khz = outcome.pre.mem_khz;
            }
        }
        ApplyOutcome::from_transaction(outcome)
    }

    /// 恢复基准（应用首次写入前的原始偏移）。
    pub fn restore_baseline(&self) -> ApplyOutcome {
        let mut state = self.state.lock().unwrap();
        let Some(baseline) = state.baseline else {
            return ApplyOutcome::failure("没有可恢复的基准：本会话尚未成功写入过 GPU 偏移".into());
        };
        let io = match ensure_session(&mut state) {
            Ok(io) => io,
            Err(err) => return ApplyOutcome::failure(err.to_string()),
        };
        match turbo::current_device(&mut state) {
            Ok(id) if id == baseline.device => (),
            Ok(_) => {
                return ApplyOutcome::failure("GPU 身份已变化，未恢复其他设备的初始偏移".into());
            }
            Err(error) => return ApplyOutcome::failure(format!("{error:#}")),
        }
        match transaction::restore_raw(io.as_ref(), &baseline.raw()) {
            Ok(()) => ApplyOutcome {
                ok: true,
                message: "已恢复 GPU 初始偏移".into(),
                core_offset_mhz: baseline.core_khz.map(|v| v / 1000),
                mem_offset_mhz: baseline.mem_khz.map(|v| v / 1000),
                rollback_error: None,
            },
            Err(error) => ApplyOutcome::failure(format!("恢复 GPU 失败：{error:#}")),
        }
    }

    /// 档案保存使用整数 MHz；失败恢复使用原始 kHz 快照。
    pub fn capture_settings(&self) -> Result<GpuClockSettings> {
        Ok(to_settings(&self.checkpoint()?.raw()))
    }

    pub fn checkpoint(&self) -> Result<GpuCheckpoint> {
        let mut state = self.state.lock().unwrap();
        let io = ensure_session(&mut state)?;
        let device = turbo::current_device(&mut state)?;
        let parsed = io.read()?;
        if !parsed.p0_present {
            return Err(anyhow!("GPU P-States 表缺少 P0，无法捕获偏移"));
        }
        Ok(GpuCheckpoint::new(device, raw_of(&parsed)))
    }

    #[cfg(test)]
    pub fn restore_checkpoint(&self, before: &GpuCheckpoint) -> Result<()> {
        let mut state = self.state.lock().unwrap();
        let io = ensure_session(&mut state)?;
        if turbo::current_device(&mut state)? != before.device {
            return Err(anyhow!("GPU 身份已变化，未恢复其他设备的偏移快照"));
        }
        transaction::restore_raw(io.as_ref(), &before.raw())
    }

    /// 档案应用：与 UI 写入同一条事务路径。
    pub fn apply_settings(&self, settings: &GpuClockSettings) -> ApplyOutcome {
        self.apply_offsets(settings.core_offset_mhz, settings.mem_offset_mhz)
    }
}

fn core_caps() -> DomainCaps {
    DomainCaps {
        min_mhz: CORE_CAP_MHZ.0,
        max_mhz: CORE_CAP_MHZ.1,
    }
}

fn mem_caps() -> DomainCaps {
    DomainCaps {
        min_mhz: MEM_CAP_MHZ.0,
        max_mhz: MEM_CAP_MHZ.1,
    }
}

fn ensure_session(state: &mut ControllerState) -> Result<Arc<dyn PstatesIo>> {
    if state.power_suspended {
        return Err(anyhow!("独显电源已关断或正在切换，GPU调校已暂停"));
    }
    connect_session(state)
}

fn disconnect_session(state: &mut ControllerState) {
    state.session = None;
    state.turbo_io = None;
    state.gpu_name = None;
}

fn connect_session(state: &mut ControllerState) -> Result<Arc<dyn PstatesIo>> {
    if let Some(io) = &state.session {
        return Ok(io.clone());
    }
    #[cfg(test)]
    {
        let io = state
            .test_source
            .clone()
            .ok_or_else(|| anyhow!("隔离GPU控制器未注入NVAPI，禁止连接真实驱动"))?;
        state.session = Some(io.clone());
        state.turbo_io = state.test_turbo.clone();
        Ok(io)
    }
    #[cfg(not(test))]
    {
        let session = NvApiSession::open()?;
        state.gpu_name = session.gpu_name().map(str::to_string);
        state.turbo_io = Some(session.clone());
        state.session = Some(session);
        Ok(state.session.clone().expect("刚刚写入"))
    }
}

fn raw_of(parsed: &ParsedPstates) -> RawOffsets {
    RawOffsets {
        core_khz: parsed.core.present.then_some(parsed.core.delta_khz),
        mem_khz: parsed.memory.present.then_some(parsed.memory.delta_khz),
    }
}

fn to_settings(raw: &RawOffsets) -> GpuClockSettings {
    GpuClockSettings {
        core_offset_mhz: raw.core_khz.map(|v| v / 1000),
        mem_offset_mhz: raw.mem_khz.map(|v| v / 1000),
    }
}

fn profile_from(
    parsed: &ParsedPstates,
    gpu_name: Option<&str>,
    state: &ControllerState,
    device: DeviceId,
) -> GpuProfile {
    let mut warnings = Vec::new();
    if parsed.count_clamped {
        warnings.push("驱动返回的 P-States 计数越界，已按容量截断读取".into());
    }
    if !parsed.p0_present {
        warnings.push("P-States 表缺少 P0（最高性能态），偏移不可用".into());
    }
    let matching_baseline = state
        .baseline
        .is_none_or(|baseline| baseline.device == device);
    if !matching_baseline {
        warnings.push("初始偏移属于其他 GPU，请连接原设备后恢复".into());
    }
    let raw = raw_of(parsed);
    let (core_min, core_max) = transaction::effective_range(&parsed.core, core_caps());
    let (mem_min, mem_max) = transaction::effective_range(&parsed.memory, mem_caps());
    GpuProfile {
        available: true,
        gpu_name: gpu_name.map(str::to_string),
        core_offset_mhz: raw.core_khz.map(|v| v / 1000),
        mem_offset_mhz: raw.mem_khz.map(|v| v / 1000),
        core_editable: parsed.editable
            && matching_baseline
            && parsed.p0_present
            && !parsed.count_clamped
            && parsed.core.editable
            && core_min <= core_max,
        mem_editable: parsed.editable
            && matching_baseline
            && parsed.p0_present
            && !parsed.count_clamped
            && parsed.memory.editable
            && mem_min <= mem_max,
        core_min_mhz: core_min,
        core_max_mhz: core_max,
        mem_min_mhz: mem_min,
        mem_max_mhz: mem_max,
        baseline: state
            .baseline
            .filter(|_| matching_baseline)
            .map(|value| to_settings(&value.raw())),
        warnings,
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// 单元测试：mock 会话注入（同文件私有字段直填），不加载 DLL、不触碰 GPU。
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "gpuctl/tests.rs"]
mod tests;
