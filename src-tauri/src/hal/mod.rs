//! 硬件抽象层（HAL）。
//!
//! 契约（AGENTS.md 工作路由）：
//! - 只经本层访问硬件；策略与命令层不感知具体通道
//! - 通道优先级见 docs/design.md §2：T1 OemDchu（InsydeDCHU.dll FFI）→
//!   T4 NVML（GPU 传感器）。T2 EC 直写 / T3 MSR 运行时由 probe-cli 与
//!   后续 Phase 提供，不在本模块
//! - 所有读数带 ok / stale 标志：`stale` = 回声检测命中（值恒等于命令号，
//!   见 docs/oem-dchu-contract.md §11-0），消费方必须把 stale 当作无效值

pub mod nvidia;
pub mod oem_dchu;

use serde::Serialize;
use std::collections::HashMap;
use std::sync::Mutex;

/// 单项读数。`stale=true` 表示疑似回声，值不可信。
#[derive(Clone, Copy, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Reading {
    pub ok: bool,
    pub value: f64,
    pub stale: bool,
}

impl Reading {
    fn bad() -> Self {
        Self { ok: false, value: 0.0, stale: false }
    }
}

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
    pub cpu_temp_c: Option<Reading>,
    pub cpu_freq_mhz: Option<Reading>,
    pub cpu_freq_max_mhz: Option<Reading>,
    pub gpu_temp_c: Option<Reading>,
    pub gpu_freq_mhz: Option<Reading>,
    pub fan: FanSnapshot,
    pub memory_used_mb: Option<Reading>,
    pub nvidia: Option<NvidiaSnapshot>,
    /// 采样时刻（毫秒）
    pub sampled_at_ms: u64,
    /// 读数异常与通道告警（界面直接展示）
    pub warnings: Vec<String>,
}

/// 回声检测器：同一命令连续返回相同值且值==命令号 → 判定 stale。
/// 依据：docs/oem-dchu-contract.md §11-0（非管理员静态回显实测）。
#[derive(Default)]
struct EchoTracker {
    table: Mutex<HashMap<i32, (i32, u32)>>,
}

impl EchoTracker {
    fn check(&self, cmd: i32, value: i32) -> bool {
        let mut table = self.table.lock().unwrap();
        let entry = table.entry(cmd).or_insert((i32::MIN, 0));
        if value == entry.0 && value == cmd {
            entry.1 = entry.1.saturating_add(1);
        } else {
            *entry = (value, 0);
        }
        entry.1 >= 2
    }
}

/// HAL 状态：DCHU 句柄 + 回声表。经 tauri manage 共享。
pub struct Hal {
    dchu: Mutex<Option<oem_dchu::Dchu>>,
    echo: EchoTracker,
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
        Self { dchu: Mutex::new(dchu), echo: EchoTracker::default(), nvidia }
    }

    fn dchu_int(&self, cmd: i32, warnings: &mut Vec<String>) -> Option<Reading> {
        let guard = self.dchu.lock().unwrap();
        let dchu = guard.as_ref()?;
        let (ok, value) = dchu.read_int(cmd);
        if !ok {
            return Some(Reading::bad());
        }
        let stale = self.echo.check(cmd, value);
        if stale {
            warnings.push(format!("DCHU cmd{cmd} 读数疑似回声（值恒为 {value}）"));
        }
        Some(Reading { ok: true, value: value as f64, stale })
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

        // cmd 12 风扇实时包（契约 §4）
        if let Some(pkg) = self.dchu_package(12) {
            let rpm = |hi: usize, lo: usize| -> u16 { ((pkg[hi] as u16) << 8) | pkg[lo] as u16 };
            let duty_pct = |at: usize| -> f64 { pkg[at] as f64 * 100.0 / 255.0 };
            if rpm(2, 3) > 0 || pkg[16] > 0 {
                snap.fan.cpu_rpm = Some(rpm(2, 3) as f64);
                snap.fan.cpu_duty_pct = Some(duty_pct(16));
            }
            if rpm(4, 5) > 0 || pkg[19] > 0 {
                snap.fan.gpu_rpm = Some(rpm(4, 5) as f64);
                snap.fan.gpu_duty_pct = Some(duty_pct(19));
            }
        }

        snap.cpu_temp_c = self.dchu_int(29, &mut warnings);
        snap.cpu_freq_mhz = self.dchu_int(16, &mut warnings);
        snap.cpu_freq_max_mhz = self.dchu_int(50, &mut warnings);
        snap.gpu_temp_c = self.dchu_int(85, &mut warnings);
        snap.gpu_freq_mhz = self.dchu_int(96, &mut warnings);
        snap.memory_used_mb = self.dchu_int(111, &mut warnings).map(|r| Reading {
            ok: r.ok,
            value: r.value / 1024.0,
            stale: r.stale,
        });

        // NVML 交叉验证：GPU 温度与 DCHU cmd85 偏差 > 20°C 时告警（契约 §11 cmd85 待验证）
        if let Some(nv) = &self.nvidia {
            let n = nv.sample();
            if let (Some(nvml_temp), Some(dchu_temp)) = (n.temp_c, snap.gpu_temp_c) {
                if dchu_temp.ok && !dchu_temp.stale && (dchu_temp.value - nvml_temp).abs() > 20.0 {
                    warnings.push(format!(
                        "cmd85 GPU 温度({:.0}°C) 与 NVML({:.0}°C) 偏差过大，cmd85 映射待验证",
                        dchu_temp.value, nvml_temp
                    ));
                }
            }
            snap.nvidia = Some(n);
        }

        if !snap.dchu_available {
            warnings.push("InsydeDCHU 通道不可用（OEM Control Center 未安装或驱动未运行）".into());
        }
        snap.warnings = warnings;
        snap
    }
}
