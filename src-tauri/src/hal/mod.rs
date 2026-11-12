//! 硬件抽象层（HAL）。
//!
//! 契约（AGENTS.md 工作路由 + docs/oem-dchu-contract.md）：
//! - 只经本层访问硬件；策略与命令层不感知具体通道
//! - **传感数据一律走 cmd12 包读**（真机验证的可靠通道）；整数读
//!   `GetDCHU_Data_Integer` 存在回声缺陷（恒返回命令号），已退役——
//!   诊断用途见 oem_dchu::Dchu::read_int（保留但不进采样路径）
//! - GPU 传感以 NVML 为准（用户态、可靠），DCHU 包读为风扇事实来源
//! - CPU 温度 = cmd12 buf[18] remote 经 OEM CalCPUTemp 换算（65W 桶，
//!   桶参数待用户对照 HWiNFO 校准，见 acceptance-checklist）

pub mod nvidia;
pub mod oem_dchu;
pub mod pawnio;

use serde::Serialize;
use std::sync::Mutex;

#[derive(Clone, Copy, Serialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct FanSnapshot {
    pub cpu_rpm: Option<f64>,
    pub cpu_duty_pct: Option<f64>,
    pub gpu_rpm: Option<f64>,
    pub gpu_duty_pct: Option<f64>,
}

#[derive(Clone, Serialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct NvidiaSnapshot {
    pub name: String,
    pub temp_c: Option<f64>,
    pub fan_pct: Option<f64>,
    pub utilization_pct: Option<f64>,
    pub mem_used_mb: Option<f64>,
    pub mem_total_mb: Option<f64>,
}

#[derive(Clone, Serialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct HardwareSnapshot {
    /// InsydeDCHU 通道是否加载成功
    pub dchu_available: bool,
    /// CPU 温度（°C，65W 桶换算；raw 见 cpu_remote_raw）
    pub cpu_temp_c: Option<f64>,
    /// cmd12 buf[18] 原始 remote 值（换算校准用）
    pub cpu_remote_raw: Option<f64>,
    pub fan: FanSnapshot,
    pub nvidia: Option<NvidiaSnapshot>,
    /// 采样时刻（毫秒）
    pub sampled_at_ms: u64,
    /// 读数异常与通道告警（界面直接展示）
    pub warnings: Vec<String>,
}

/// OEM CalCPUTemp 换算（契约 §9）。65W 桶 = i5-13500HX 最近档。
fn cpu_remote_to_celsius(raw: f64) -> f64 {
    if raw <= 50.0 {
        raw - 1.0
    } else {
        (raw - 35.0) * 0.41 + 43.7
    }
}

/// HAL 状态：DCHU 句柄。经 tauri manage 共享。
pub struct Hal {
    dchu: Mutex<Option<oem_dchu::Dchu>>,
    nvidia: Option<nvidia::Nvidia>,
}

impl Hal {
    pub fn new() -> Self {
        let dchu = match oem_dchu::Dchu::load() {
            Ok(d) => {
                eprintln!("[hal] InsydeDCHU 已加载");
                Some(d)
            }
            Err(err) => {
                eprintln!("[hal] InsydeDCHU 不可用: {err}");
                None
            }
        };
        let nvidia = match nvidia::Nvidia::init() {
            Ok(n) => Some(n),
            Err(err) => {
                eprintln!("[hal] NVML 不可用: {err}");
                None
            }
        };
        Self { dchu: Mutex::new(dchu), nvidia }
    }

    fn dchu_package(&self, cmd: i32) -> Option<[u8; 256]> {
        let guard = self.dchu.lock().unwrap();
        guard.as_ref().map(|d| d.read_package(cmd))
    }

    /// 受控写入口：仅在通道可用时执行（fanctl / powerctl 使用）。
    /// 写入合法性由调用方经 ccc-core 校验后保证；闭包返回 Result 以便 `?`。
    pub fn with_dchu<T>(
        &self,
        f: impl FnOnce(&oem_dchu::Dchu) -> anyhow::Result<T>,
    ) -> anyhow::Result<T> {
        let guard = self.dchu.lock().unwrap();
        let dchu = guard
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("InsydeDCHU 通道不可用"))?;
        f(dchu)
    }

    /// 一次完整采样。永不 panic：任何通道失败都体现为 None/警告。
    pub fn snapshot(&self) -> HardwareSnapshot {
        let mut warnings: Vec<String> = Vec::new();
        let mut snap = HardwareSnapshot {
            dchu_available: self.dchu.lock().unwrap().is_some(),
            sampled_at_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0),
            ..Default::default()
        };

        // cmd 12 包：风扇 RPM/duty + CPU remote 温度（全部传感事实来源）
        if let Some(pkg) = self.dchu_package(12) {
            let rpm = |hi: usize, lo: usize| -> u16 { ((pkg[hi] as u16) << 8) | pkg[lo] as u16 };
            let duty_pct = |at: usize| -> f64 { pkg[at] as f64 * 100.0 / 255.0 };
            if rpm(2, 3) > 0 {
                snap.fan.cpu_rpm = Some(rpm(2, 3) as f64);
                snap.fan.cpu_duty_pct = Some(duty_pct(16));
            }
            if rpm(4, 5) > 0 {
                snap.fan.gpu_rpm = Some(rpm(4, 5) as f64);
                snap.fan.gpu_duty_pct = Some(duty_pct(19));
            }
            let raw_remote = pkg[18] as f64;
            snap.cpu_temp_c = Some(cpu_remote_to_celsius(raw_remote).round());
            snap.cpu_remote_raw = Some(raw_remote);
        }

        // NVML：GPU 事实来源；与 DCHU GPU remote 交叉验证留待 UI（契约 §11 cmd85）
        if let Some(nv) = &self.nvidia {
            snap.nvidia = Some(nv.sample());
        }

        if !snap.dchu_available {
            warnings.push("InsydeDCHU 通道不可用（OEM Control Center 未安装或驱动未运行）".into());
        }
        if snap.fan.cpu_rpm.is_none() && snap.dchu_available {
            warnings.push("cmd12 包读为空——风扇传感异常".into());
        }
        snap.warnings = warnings;
        snap
    }
}
