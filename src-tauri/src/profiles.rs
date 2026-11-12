//! 性能档案域（Phase 3）：把「风扇曲线 + 模式 + PL + 电压偏移」打包为
//! JSON 档案，一键应用。存储于应用数据目录 profiles/*.json。
//!
//! 应用顺序即安全顺序：曲线（cmd14）→ 模式（cmd121）→ PL 持久化 →
//! 电压偏移持久化；每步独立报告成败，失败不中断后续步骤（结果可见）。
//! 运行时 MSR 通道不进档案流水线（档案 = 可恢复意图，持久化通道语义）。

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use tauri::Manager;

use crate::fanctl::{self, FAN_MODE_CUSTOM};
use crate::hal::Hal;

const FAN_IDS: [&str; 3] = ["cpu", "gpu1", "gpu2"];

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct ProfileFanPoints {
    pub t2: f64,
    pub d2_pct: f64,
    pub t3: f64,
    pub d3_pct: f64,
}

impl Default for ProfileFanPoints {
    fn default() -> Self {
        Self { t2: 60.0, d2_pct: 40.0, t3: 80.0, d3_pct: 70.0 }
    }
}

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ProfileSettings {
    pub fan_mode: Option<u8>,
    pub fan_curves: Option<HashMap2>,
    pub pl1_w: Option<f64>,
    pub pl2_w: Option<f64>,
    pub core_offset_mv: Option<i32>,
    pub gt_offset_mv: Option<i32>,
}

type HashMap2 = std::collections::BTreeMap<String, ProfileFanPoints>;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileFile {
    pub version: u32,
    pub name: String,
    pub saved_at_ms: u64,
    #[serde(default)]
    pub settings: ProfileSettings,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileMeta {
    pub name: String,
    pub saved_at_ms: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyStep {
    pub step: String,
    pub ok: bool,
    pub message: String,
}

fn sanitize_name(name: &str) -> Result<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed.len() > 60 {
        return Err(anyhow!("档案名长度需在 1–60 字符"));
    }
    if !trimmed.chars().all(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.')) {
        return Err(anyhow!("档案名只能包含文字、数字、空格、- _ ."));
    }
    Ok(trimmed.to_string())
}

fn profiles_dir(app: &tauri::AppHandle) -> Result<std::path::PathBuf> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|_| anyhow!("应用数据目录不可用"))?
        .join("profiles");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn profile_path(app: &tauri::AppHandle, name: &str) -> Result<std::path::PathBuf> {
    let safe = sanitize_name(name)?;
    Ok(profiles_dir(app)?.join(format!("{safe}.json")))
}

pub fn save(app: &tauri::AppHandle, settings: ProfileSettings, name: &str) -> Result<()> {
    let path = profile_path(app, name)?;
    let file = ProfileFile {
        version: 1,
        name: sanitize_name(name)?,
        saved_at_ms: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0),
        settings,
    };
    std::fs::write(&path, serde_json::to_vec_pretty(&file)?)
        .with_context(|| format!("写入 {} 失败", path.display()))
}

pub fn list(app: &tauri::AppHandle) -> Result<Vec<ProfileMeta>> {
    let dir = profiles_dir(app)?;
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir)?.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        if let Ok(bytes) = std::fs::read(&path) {
            if let Ok(file) = serde_json::from_slice::<ProfileFile>(&bytes) {
                out.push(ProfileMeta { name: file.name, saved_at_ms: file.saved_at_ms });
            }
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

pub fn load(app: &tauri::AppHandle, name: &str) -> Result<ProfileFile> {
    let path = profile_path(app, name)?;
    let bytes = std::fs::read(&path).with_context(|| format!("读取 {} 失败", path.display()))?;
    serde_json::from_slice(&bytes).context("档案 JSON 解析失败")
}

pub fn delete(app: &tauri::AppHandle, name: &str) -> Result<()> {
    let path = profile_path(app, name)?;
    std::fs::remove_file(path).context("删除档案失败")
}

/// 应用档案流水线：每步独立报告，失败继续后续步骤。
pub fn apply(app: &tauri::AppHandle, hal: &Hal, name: &str) -> Result<Vec<ApplyStep>> {
    let file = load(app, name)?;
    let mut steps = Vec::new();
    let settings = &file.settings;

    // 1. 风扇曲线
    if let Some(curves) = &settings.fan_curves {
        let pick = |id: &str| -> Option<ccc_core::fan::OemCurvePoints> {
            curves.get(id).map(|p| ccc_core::fan::OemCurvePoints {
                t2: p.t2,
                d2: p.d2_pct,
                t3: p.t3,
                d3: p.d3_pct,
            })
        };
        let result = fanctl::apply_curve(hal, pick("cpu"), pick("gpu1"), pick("gpu2"));
        steps.push(step("风扇曲线", result));
    }

    // 2. 风扇模式
    if let Some(mode) = settings.fan_mode {
        let result = fanctl::set_mode(hal, mode);
        steps.push(step(
            &format!("风扇模式({mode})"),
            result.map_err(|e| e.into()),
        ));
    }

    // 3. PL 持久化
    if settings.pl1_w.is_some() || settings.pl2_w.is_some() {
        let result = crate::powerctl::apply_pl_persist(hal, settings.pl1_w, settings.pl2_w);
        steps.push(step("功耗墙(page6)", result));
    }

    // 4. 电压偏移持久化
    if settings.core_offset_mv.is_some() || settings.gt_offset_mv.is_some() {
        let result = crate::powerctl::apply_voltage_persist(
            hal,
            settings.core_offset_mv,
            settings.gt_offset_mv,
        );
        steps.push(step("电压偏移(page6)", result));
    }

    if steps.is_empty() {
        steps.push(ApplyStep {
            step: "无内容".into(),
            ok: false,
            message: "档案不包含任何设置".into(),
        });
    }
    Ok(steps)
}

/// 从当前固件状态快照一份档案设置（fan 曲线来自 page4/cmd13，PL/电压来自 page6）。
pub fn capture_current(hal: &Hal) -> Result<ProfileSettings> {
    let profile = fanctl::read_profile(hal)?;
    let power = crate::powerctl::read_profile(hal)?;
    let mut curves = HashMap2::new();
    for (id, _) in FAN_IDS.iter().enumerate() {
        let set = &profile.fans[id];
        curves.insert(
            set.id.clone(),
            ProfileFanPoints { t2: set.t2, d2_pct: set.d2_pct, t3: set.t3, d3_pct: set.d3_pct },
        );
    }
    Ok(ProfileSettings {
        fan_mode: Some(profile.current_mode),
        fan_curves: Some(curves),
        pl1_w: power.pl1_w,
        pl2_w: power.pl2_w,
        core_offset_mv: power.core_offset_mv.map(|v| v as i32),
        gt_offset_mv: power.gt_offset_mv.map(|v| v as i32),
    })
}

fn step(name: &str, result: Result<()>) -> ApplyStep {
    match result {
        Ok(()) => ApplyStep { step: name.into(), ok: true, message: "成功".into() },
        Err(err) => ApplyStep { step: name.into(), ok: false, message: err.to_string() },
    }
}

/// FAN_MODE_CUSTOM 保留引用（档案默认模式语义提示用）。
#[allow(dead_code)]
const _: u8 = FAN_MODE_CUSTOM;
