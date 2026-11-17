//! 特权工作进程的原厂顺序与补偿；通过Io显式注入，不接触Tauri。
use super::model::{Device, Plan, Target};
use anyhow::{Result, anyhow, bail};
use serde::{Deserialize, Serialize};
#[cfg(test)]
#[path = "operation/tests.rs"]
mod tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum Phase {
    Request = 1,
    Lease,
    Elevation,
    Open,
    Preflight,
    FirmwareOn,
    NodeOn,
    NodeOff,
    FirmwareOff,
    Save,
    Verify,
}
#[derive(Debug)]
pub struct Failure {
    pub phase: Phase,
    pub message: String,
    pub code: u16,
    pub recovery_failed: bool,
}
impl Failure {
    pub fn new(phase: Phase, error: impl std::fmt::Display) -> Self {
        Self {
            phase,
            message: error.to_string(),
            code: 0,
            recovery_failed: false,
        }
    }
    pub fn exit_code(&self) -> u32 {
        0x4300_0000
            | ((self.phase as u32 | if self.recovery_failed { 128 } else { 0 }) << 16)
            | u32::from(self.code)
    }
    pub fn describe(code: u32) -> String {
        if code >> 24 != 0x43 || !(1..=11).contains(&((code >> 16) & 127)) {
            return format!("原厂独显工作进程异常退出（0x{code:08X}）");
        }
        let phase = match (code >> 16) & 127 {
            1 => "工作参数校验",
            2 => "控制权保留",
            3 => "管理员权限检查",
            4 => "原厂通道连接",
            5 => "目标与前态检查",
            6 => "固件开启命令",
            7 => "控制节点开启",
            8 => "控制节点关闭",
            9 => "固件关闭命令",
            10 => "自动关断策略保存",
            11 => "状态回读",
            _ => "未知阶段",
        };
        format!(
            "原厂独显{phase}失败（代码{}）；{}",
            code & 65535,
            if code & 0x0080_0000 != 0 {
                "补偿未完成，请重试恢复初始状态"
            } else {
                "未写入或已补偿本次前态"
            }
        )
    }
}
pub trait Io {
    fn inspect(&self, id: &str) -> Result<Device>;
    fn firmware(&self, enabled: bool) -> Result<()>;
    fn node(&self, id: &str, enabled: bool) -> Result<()>;
    fn save(&self, automatic_power_off: bool) -> Result<()>;
    fn pause(&self) {}
}
fn step(phase: Phase, result: Result<()>) -> std::result::Result<(), Failure> {
    result.map_err(|error| {
        let mut failure = Failure::new(phase, format!("{error:#}"));
        if let Some(code) = error.downcast_ref::<NativeCode>() {
            failure.code = code.0.try_into().unwrap_or(u16::MAX);
        }
        failure
    })
}
#[derive(Debug)]
pub struct NativeCode(pub u32);
impl std::fmt::Display for NativeCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Configuration Manager错误{}", self.0)
    }
}
impl std::error::Error for NativeCode {}
pub fn preflight(device: &Device, expected: Target, target: Target) -> Result<()> {
    if !device.healthy() {
        bail!("原厂控制节点存在其他设备故障");
    }
    if device.target() != expected {
        bail!("原厂独显前态已变化，请重新读取并确认");
    }
    if device.target() != target && !target.enabled && !device.can_disable {
        bail!("独显输出在使用中或缺少可用集显输出，不能关断");
    }
    Ok(())
}
fn verify(io: &impl Io, id: &str, target: Target) -> Result<()> {
    let current = io.inspect(id)?;
    if !current.healthy() || current.target() != target {
        bail!("原厂独显控制节点或策略回读不一致");
    }
    Ok(())
}
fn switch(io: &impl Io, id: &str, target: Target) -> std::result::Result<(), Failure> {
    if target.enabled {
        step(Phase::FirmwareOn, io.firmware(true))?;
        io.pause();
        step(Phase::NodeOn, io.node(id, true))?;
        io.pause();
    } else {
        let mut last = None;
        for _ in 0..3 {
            last = io.node(id, false).err();
            io.pause();
        }
        let current = io
            .inspect(id)
            .map_err(|e| Failure::new(Phase::NodeOff, e))?;
        if current.enabled || !current.healthy() {
            step(
                Phase::NodeOff,
                Err(last.unwrap_or_else(|| anyhow!("控制节点未关闭"))),
            )?;
        }
        if !current.can_disable {
            return Err(Failure::new(Phase::Preflight, "关闭期间显示输出已变化"));
        }
        step(Phase::FirmwareOff, io.firmware(false))?;
    }
    step(Phase::Save, io.save(target.automatic_power_off))?;
    step(Phase::Verify, verify(io, id, target))
}
pub fn execute(io: &impl Io, plan: &Plan) -> std::result::Result<(), Failure> {
    execute_checked(io, plan, false)
}
pub(crate) fn replay(io: &impl Io, plan: &Plan) -> std::result::Result<(), Failure> {
    execute_checked(io, plan, true)
}
fn execute_checked(io: &impl Io, plan: &Plan, restore: bool) -> std::result::Result<(), Failure> {
    plan.validate()
        .map_err(|e| Failure::new(Phase::Request, e))?;
    let before = io
        .inspect(&plan.id)
        .map_err(|e| Failure::new(Phase::Preflight, e))?;
    if before.id != plan.id {
        return Err(Failure::new(Phase::Preflight, "原厂控制节点身份不一致"));
    }
    preflight(&before, plan.expected, plan.target)
        .map_err(|e| Failure::new(Phase::Preflight, e))?;
    if restore && !plan.target.enabled && !before.can_disable {
        return Err(Failure::new(Phase::Preflight, "恢复关断会影响当前显示输出"));
    }
    if before.target() == plan.target && !restore {
        return Ok(());
    }
    if let Err(mut failure) = switch(io, &plan.id, plan.target) {
        let recovery = io.inspect(&plan.id).and_then(|current| {
            if current.target() == before.target()
                && current.healthy()
                && failure.phase == Phase::NodeOff
            {
                return Ok(());
            }
            if !before.enabled && !current.can_disable {
                bail!("恢复关断会影响当前显示输出");
            }
            switch(io, &plan.id, before.target()).map_err(|e| anyhow!(e.message))
        });
        if let Err(error) = recovery {
            failure.recovery_failed = true;
            failure.message.push_str(&format!("；恢复失败：{error:#}"));
        }
        return Err(failure);
    }
    Ok(())
}
