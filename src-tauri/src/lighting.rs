//! 键盘灯光域：cmd103 九模式、独立三区颜色/开关和 cmd39 白光。
//!
//! 协议事实：反编译静态审查（未真机验证，布局见 docs/keyboard-contract.md）——kbled 6.26
//! 为主，CC40 7.19 的特殊编码差异单独记录。apply = cmd 直写 + page2 字段事务：
//! 预读 → 写入 → 回读校验 → 失败自动恢复）。首次写入前 LightingController 先捕获 page2
//! 基线块；失败回放本次快照，用户恢复回放首次模式及其颜色编码。立即通道无可靠回读
//! （Integer 回声缺陷，oem-dchu-contract.md §11-0）：结果只声称"已写入"不声称"已生效"。

use std::sync::Mutex;

use anyhow::{Result, anyhow};
use ccc_core::settings::{self, Patch};

use crate::hal::{Hal, oem_dchu::Dchu};

#[path = "lighting/protocol.rs"]
mod protocol;
use protocol::*;
pub use protocol::{ApplyOutcome, KeyboardFamily, LightingProfile, LightingRequest};
#[path = "lighting/brightness_event.rs"]
pub(crate) mod brightness_event;
#[path = "lighting/color_cycle.rs"]
pub(crate) mod color_cycle;
#[path = "lighting/hotkey.rs"]
pub(crate) mod hotkey;
#[path = "lighting/lightbar.rs"]
pub mod lightbar;
#[path = "lighting/multizone.rs"]
pub mod multizone;
#[path = "lighting/options.rs"]
pub mod options;
#[path = "lighting/perkey.rs"]
pub mod perkey;
#[path = "lighting/scope.rs"]
mod scope;
#[path = "lighting/transaction.rs"]
mod transaction;
#[path = "lighting/zone_hotkey.rs"]
pub(crate) mod zone_hotkey;
pub use scope::Target;
pub(crate) use scope::{Plan, Scope};

// ---- IO 抽象（控制器流程可离线测试；唯一实现走 Hal → Dchu）----

trait LightingIo {
    fn white_brightness(&self) -> Result<i32>;
    fn cycle_info(&self, raw_type: u8) -> Result<color_cycle::Info>;
    fn cycle_index(&self) -> Result<u8>;
    fn light_bar(&self, raw_type: Option<u8>) -> Result<bool>;
    /// SetDCHU_Data 4B LE 整字写（依赖 Hal 侧 Dchu::set_u32）。
    fn write_word(&self, command: i32, value: u32) -> Result<()>;
    fn read_saved_block(&self) -> Result<SavedBlock>;
    /// page2 字段事务：预读 → 写入 → 回读校验 → 失败自动恢复（ccc-core）。
    fn apply_saved(&self, patches: &[Patch]) -> Result<()>;
}

struct DchuLighting<'a>(&'a Hal);

impl LightingIo for DchuLighting<'_> {
    fn white_brightness(&self) -> Result<i32> {
        self.0.with_dchu(|d| {
            let (ok, value) = d.read_int(61);
            if !ok {
                anyhow::bail!("白光亮度报告读取失败或为回声：{value}");
            }
            Ok(value)
        })
    }
    fn cycle_info(&self, raw_type: u8) -> Result<color_cycle::Info> {
        color_cycle::native_info(self.0, raw_type)
    }
    fn cycle_index(&self) -> Result<u8> {
        self.0.with_dchu(|d| Ok(d.read_settings(2, 127, 1)?[0]))
    }
    fn light_bar(&self, raw_type: Option<u8>) -> Result<bool> {
        self.0.with_dchu(|d| {
            let caps = options::native::capabilities(d)?;
            if raw_type != Some(caps.keyboard_type) {
                anyhow::bail!("键盘类型已变化，请重新读取");
            }
            if !caps.peripheral_support_known {
                anyhow::bail!("外围灯区能力未知，不能确定共享开关");
            }
            Ok(caps.light_bar)
        })
    }
    fn write_word(&self, command: i32, value: u32) -> Result<()> {
        self.0.with_dchu(|d| d.set_u32(command, value))
    }
    fn read_saved_block(&self) -> Result<SavedBlock> {
        self.0.with_dchu(|d| {
            SavedBlock::from_bytes(d.read_settings(
                i32::from(PAGE),
                BLOCK_OFFSET as i32,
                BLOCK_LEN,
            )?)
        })
    }
    fn apply_saved(&self, patches: &[Patch]) -> Result<()> {
        self.0.with_dchu(|d| {
            settings::apply(d, PAGE, patches, None)
                .map(|_| ())
                .map_err(|e| anyhow!(e))
        })
    }
}

// ---- 控制器（tauri manage 单例；baseline = 本应用首次写入前的 page2 块）----

#[derive(Clone)]
struct Baseline {
    family: KeyboardFamily,
    raw_type: Option<u8>,
    light_bar: bool,
    block: Vec<u8>,
    cycle_index: Option<u8>,
}

struct ApplyError {
    message: String,
    rollback: Option<String>,
}

#[derive(Default)]
pub struct LightingController {
    baseline: Mutex<Option<Baseline>>,
}

fn to_outcome(result: Result<(), ApplyError>) -> ApplyOutcome {
    match result {
        Ok(()) => ApplyOutcome {
            ok: true,
            message: "键盘灯已写入（立即通道无回读，效果以实际灯光为准）".into(),
            rollback_error: None,
        },
        Err(error) => ApplyOutcome {
            ok: false,
            message: error.message,
            rollback_error: error.rollback,
        },
    }
}

impl LightingController {
    /// 读侧事实：判型检测 + page2 当前保存值 + 基线。永不 Err（可用性折叠）。
    pub fn read_profile(&self, hal: &Hal) -> LightingProfile {
        let read = || -> Result<LightingProfile> {
            let scope = self.scope()?;
            let _gate = hal.begin_write()?;
            let baseline = scope.baseline.as_ref().and_then(|b| {
                SavedBlock::from_bytes(b.block.clone())
                    .ok()
                    .map(|block| block.snapshot())
            });
            hal.with_dchu(|d| Ok(build_profile(d, baseline)))
        };
        match read() {
            Ok(profile) => profile,
            Err(error) => LightingProfile {
                available: false,
                unavailable_reason: Some(format!("{error:#}")),
                ..Default::default()
            },
        }
    }

    pub fn apply(&self, hal: &Hal, request: LightingRequest) -> ApplyOutcome {
        let run = || -> Result<ApplyOutcome> {
            let mut scope = self.scope()?;
            let _gate = hal.begin_write()?;
            let current = scope
                .capture(hal)?
                .ok_or_else(|| anyhow!("检测到的键盘类型不支持当前灯光命令"))?;
            if current.settings.family() != request.family() {
                anyhow::bail!("请求类型与检测到的键盘类型不一致");
            }
            let target = Target {
                keyboard_type: current.keyboard_type,
                enabled: request.enabled(),
                settings: request,
            };
            Ok(to_outcome(transaction::apply_target(
                &DchuLighting(hal),
                &mut scope.baseline,
                &target,
                &mut false,
            )))
        };
        run().unwrap_or_else(|e| failure(format!("{e:#}")))
    }

    pub fn restore(&self, hal: &Hal) -> ApplyOutcome {
        let run = || -> Result<ApplyOutcome> {
            let scope = self.scope()?;
            let _gate = hal.begin_write()?;
            let initial = scope
                .baseline
                .as_ref()
                .ok_or_else(|| anyhow!("没有键盘灯初始快照"))?;
            scope::require_device(hal, initial.raw_type, initial.family)?;
            Ok(transaction::restore_initial(
                &DchuLighting(hal),
                &scope.baseline,
            ))
        };
        run().unwrap_or_else(|e| failure(format!("{e:#}")))
    }

    fn apply_io(
        &self,
        io: &impl LightingIo,
        baseline: &mut Option<Baseline>,
        request: &LightingRequest,
        raw_type: Option<u8>,
    ) -> ApplyOutcome {
        to_outcome(transaction::apply_request(io, baseline, request, raw_type))
    }
}

fn build_profile(dchu: &Dchu, baseline: Option<KeyboardSnapshot>) -> LightingProfile {
    let mut warnings = Vec::new();
    // 判型（kbled BaseKB.GetKBType 同款）：cmd70 Init_WMI 触发（Integer 回读值不可信，仅取
    // 触发作用）后读 GetDCHU_Data_Buffer(13)[15]；cmd16 与 CPU 频率双占位，不作判型依据。
    let _ = dchu.read_int(70);
    let live = dchu.read_package(13).ok().map(|package| package[15]);
    let cached = dchu
        .read_settings(i32::from(PAGE), 4, 1)
        .ok()
        .and_then(|bytes| bytes.first().copied());
    let (detected_type, detected_family, detection_source, note) = select_detection(live, cached);
    if let Some(note) = note {
        warnings.push(note);
    }
    if matches!(detected_type, Some(6 | 22)) {
        warnings.push(
            "颜色编码采用 LedKeyboardSetting 6.26；CC40 的特殊型号编码不同，详见键盘协议文档"
                .into(),
        );
    }
    warnings.push(
        "立即通道写入无可靠回读（Integer 回声缺陷）：结果仅表示已写入，效果以实际灯光为准".into(),
    );
    let current = dchu
        .read_settings(i32::from(PAGE), BLOCK_OFFSET as i32, BLOCK_LEN)
        .ok()
        .and_then(|bytes| SavedBlock::from_bytes(bytes).ok())
        .map(|block| block.snapshot());
    if current.is_none() {
        warnings.push("page2 键盘灯保存块读取失败：当前保存值未知".into());
    }
    let unavailable_reason = if detected_family == KeyboardFamily::ZoneRgb {
        options::native::capabilities(dchu)
            .and_then(|caps| {
                if !caps.peripheral_support_known {
                    anyhow::bail!("外围灯区能力未知，不能确定共享开关");
                }
                Ok(())
            })
            .err()
            .map(|error| format!("{error:#}"))
    } else {
        None
    };
    LightingProfile {
        available: unavailable_reason.is_none(),
        unavailable_reason,
        detected_type,
        detected_family,
        detection_source,
        current,
        baseline,
        warnings,
    }
}

/// 判定次序（绝不猜测 RGB）：live 活动类型 → OEM page2 缓存 → live"无背光" → 未知。
pub(crate) fn select_detection(
    live: Option<u8>,
    cached: Option<u8>,
) -> (Option<u8>, KeyboardFamily, Option<String>, Option<String>) {
    let active = |raw: u8| {
        matches!(
            family_from_type(raw),
            KeyboardFamily::ZoneRgb
                | KeyboardFamily::White
                | KeyboardFamily::Perkey
                | KeyboardFamily::MultiZone
        )
    };
    if let Some(raw) = live.filter(|raw| active(*raw)) {
        return (Some(raw), family_from_type(raw), Some("wmi13".into()), None);
    }
    if let Some(raw) = cached.filter(|raw| active(*raw)) {
        let note = live.map(|_| {
            "live 判型不可信（读数 0 或未知值），改用 OEM page2 缓存（offset 4）".to_string()
        });
        return (
            Some(raw),
            family_from_type(raw),
            Some("page2Cache".into()),
            note,
        );
    }
    if let Some(raw) = live.filter(|raw| family_from_type(*raw) == KeyboardFamily::None) {
        return (Some(raw), KeyboardFamily::None, Some("wmi13".into()), None);
    }
    (
        live.or(cached),
        KeyboardFamily::Unknown,
        Some("unavailable".into()),
        Some(
            "未能可靠检测键盘类型（WMI13 包与 page2 缓存均不可用或不识别）；保留未知，未开放灯光写入"
                .into(),
        ),
    )
}

#[cfg(test)]
#[path = "lighting/tests.rs"]
mod tests;
