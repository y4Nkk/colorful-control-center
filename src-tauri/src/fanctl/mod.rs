//! 风扇控制域（Phase 2，OEM 曲线模式）。
//!
//! 语义全部来自 docs/oem-dchu-contract.md §5/§6：
//! - 自定义曲线 = cmd14 包（每风扇 T2/D2、T3/D3 可调 + 固件斜率）+
//!   风扇模式置 Custom(6) + page4 持久化
//! - 模式切换 = cmd121 sub1；偏移 = sub14；恢复默认 = sub34 + 模式 0
//! - 固件执行曲线：软件崩溃不影响风扇（安全设计，无需软件看门狗）
//!
//! 所有写入经 ccc-core validate/build（钳位 ≥20%），禁止绕过。

use anyhow::{anyhow, Context, Result};
use serde::Serialize;

use crate::hal::Hal;
use ccc_core::fan::{
    build_curve_package, oem_page4_offsets, validate_oem_curve, OemCurvePoints, OemFanAnchors,
    OemFanCurveInput, MAX_DUTY_PCT,
};

/// 风扇模式（契约 §7，本机确认语义）
pub const FAN_MODE_AUTO: u8 = 0;
pub const FAN_MODE_MAX: u8 = 1;
pub const FAN_MODE_MAXQ: u8 = 5;
pub const FAN_MODE_CUSTOM: u8 = 6;
pub const FAN_MODE_SLOW: u8 = 8;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FanPointSet {
    /// cpu | gpu1 | gpu2
    pub id: String,
    /// 存在（cmd13 bits_FanExist）
    pub present: bool,
    /// 固件固定端点
    pub t1: f64,
    pub d1_pct: f64,
    /// 可调三点（cmd13 默认值；page4 有持久化值时以持久化为准）
    pub t2: f64,
    pub d2_pct: f64,
    pub t3: f64,
    pub d3_pct: f64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FanProfile {
    pub fan_count: u8,
    pub custom_supported: bool,
    pub current_mode: u8,
    pub offset_pct: u8,
    pub fans: Vec<FanPointSet>,
}

/// 读取风扇配置（cmd13 默认曲线 + page4 当前持久化值/模式/偏移）。
pub fn read_profile(hal: &Hal) -> Result<FanProfile> {
    hal.with_dchu(|dchu| {
        let pkg = dchu.read_package(13);
        let fan_count = pkg[12];
        let fan_exist = pkg[50];
        let custom_supported = fan_count > 1 || (pkg[43] >> 1) & 1 == 0;

        let page4 = dchu.read_settings(4, 0, 256);
        let present = |bit: usize| -> bool { (fan_exist >> bit) & 1 == 1 };

        let mut fans = Vec::new();
        for (index, id) in ["cpu", "gpu1", "gpu2"].iter().enumerate() {
            let base = 16 + index * 8; // cmd13 中每风扇 T1,D1,T2,D2,T3,D3 六字节
            let mut set = FanPointSet {
                id: (*id).to_string(),
                present: present(index),
                t1: pkg[base] as f64,
                d1_pct: pkg[base + 1] as f64 * 100.0 / 255.0,
                t2: pkg[base + 2] as f64,
                d2_pct: pkg[base + 3] as f64 * 100.0 / 255.0,
                t3: pkg[base + 4] as f64,
                d3_pct: pkg[base + 5] as f64 * 100.0 / 255.0,
            };
            // page4 持久化优先（用户上次自定义的值；0 表示从未写过）
            let (d_base, t_base, _) = oem_page4_offsets(index);
            let d2_saved = page4[d_base + 1];
            if d2_saved != 0 {
                set.t2 = page4[t_base + 1] as f64;
                set.d2_pct = d2_saved as f64;
                set.t3 = page4[t_base + 2] as f64;
                set.d3_pct = page4[d_base + 2] as f64;
            }
            fans.push(set);
        }

        Ok(FanProfile {
            fan_count,
            custom_supported,
            current_mode: page4[5],
            offset_pct: page4[7],
            fans,
        })
    })
    .context("读取风扇配置失败")
}

/// 应用自定义曲线：三个风扇各自的 T2/D2/T3/D3（None = 保持现值）。
/// 顺序：校验 → cmd14 写包 → 模式置 Custom → page4 持久化。
pub fn apply_curve(
    hal: &Hal,
    cpu: Option<OemCurvePoints>,
    gpu1: Option<OemCurvePoints>,
    gpu2: Option<OemCurvePoints>,
) -> Result<()> {
    let profile = read_profile(hal)?;
    if !profile.custom_supported {
        return Err(anyhow!("本机固件未启用自定义风扇曲线（cmd13 能力位）"));
    }
    let pick = |id: &str, want: Option<OemCurvePoints>| -> Result<OemFanCurveInput> {
        let set = profile
            .fans
            .iter()
            .find(|f| f.id == id)
            .ok_or_else(|| anyhow!("缺少 {id} 风扇信息"))?;
        let points = match want {
            Some(p) => p,
            None => OemCurvePoints { t2: set.t2, d2: set.d2_pct, t3: set.t3, d3: set.d3_pct },
        };
        let input = OemFanCurveInput {
            anchors: OemFanAnchors { t1: set.t1, d1: set.d1_pct },
            points,
        };
        validate_oem_curve(&input).map_err(|e| anyhow!("{id}: {e}"))?;
        Ok(input)
    };

    let cpu = pick("cpu", cpu)?;
    let gpu1 = pick("gpu1", gpu1)?;
    let gpu2 = pick("gpu2", gpu2)?;
    let package = build_curve_package(&cpu, &gpu1, &gpu2);

    hal.with_dchu(|dchu| {
        dchu.set_package(14, &package)?;
        set_mode_internal(dchu, FAN_MODE_CUSTOM)?;
        persist_page4(dchu, &[(0, &cpu), (1, &gpu1), (2, &gpu2)])?;
        Ok(())
    })
}

fn set_mode_internal(dchu: &crate::hal::oem_dchu::Dchu, mode: u8) -> Result<()> {
    dchu.set_data_sub(121, 1, mode as u32)?;
    let page4 = dchu.read_settings(4, 0, 256);
    let mut patched = page4.clone();
    patched[5] = mode;
    dchu.write_settings(4, 0, &patched)
}

/// page4 持久化：读 256B → 按扇区补丁 → 原子写回。
fn persist_page4(
    dchu: &crate::hal::oem_dchu::Dchu,
    fans: &[(usize, &OemFanCurveInput)],
) -> Result<()> {
    let page4 = dchu.read_settings(4, 0, 256);
    let mut patched = page4.clone();
    for (index, input) in fans {
        let p = &input.points;
        let (d_base, t_base, s_base) = oem_page4_offsets(*index);
        patched[d_base] = input.anchors.d1.round() as u8;
        patched[d_base + 1] = p.d2.round() as u8;
        patched[d_base + 2] = p.d3.round() as u8;
        patched[d_base + 3] = MAX_DUTY_PCT as u8;
        patched[t_base] = input.anchors.t1.round() as u8;
        patched[t_base + 1] = p.t2.round() as u8;
        patched[t_base + 2] = p.t3.round() as u8;
        patched[t_base + 3] = 100;
        let slopes = [
            ccc_core::fan::oem_slope(input.anchors.d1, p.d2, input.anchors.t1, p.t2),
            ccc_core::fan::oem_slope(p.d2, p.d3, p.t2, p.t3),
            ccc_core::fan::oem_slope(p.d3, MAX_DUTY_PCT, p.t3, 100.0),
        ];
        for (i, slope) in slopes.iter().enumerate() {
            let bytes = (*slope as u16).to_le_bytes();
            patched[s_base + i * 2] = bytes[0];
            patched[s_base + i * 2 + 1] = bytes[1];
        }
    }
    dchu.write_settings(4, 0, &patched)
}

/// 切换风扇模式（0=自动 1=全速 5=MaxQ 6=自定义 8=静音）。
pub fn set_mode(hal: &Hal, mode: u8) -> Result<()> {
    hal.with_dchu(|dchu| set_mode_internal(dchu, mode))
}

/// 风扇偏移（%，OEM 语义 0–100 正向），经 *255/100 编码。
pub fn set_offset(hal: &Hal, pct: u32) -> Result<()> {
    let clamped = pct.clamp(0, 100) as f64;
    let encoded = (255.0 * (clamped / 100.0)).round() as u32;
    hal.with_dchu(|dchu| {
        dchu.set_data_sub(121, 14, encoded)?;
        let page4 = dchu.read_settings(4, 0, 256);
        let mut patched = page4.clone();
        patched[7] = clamped as u8;
        dchu.write_settings(4, 0, &patched)
    })
}

/// 恢复固件默认曲线（cmd121 sub34）并切回自动模式。
pub fn restore_default(hal: &Hal) -> Result<()> {
    hal.with_dchu(|dchu| {
        dchu.set_data_sub(121, 34, 1)?;
        set_mode_internal(dchu, FAN_MODE_AUTO)
    })
}
