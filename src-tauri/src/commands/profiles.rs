//! 性能档案命令层（薄委托 → profiles；应用流水线每步独立报告）。

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use crate::hal::Hal;
use crate::profiles::{ApplyStep, ProfileMeta, ProfileSettings};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveProfileArgs {
    pub name: String,
    pub settings: ProfileSettings,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpResult {
    pub ok: bool,
    pub message: String,
}

#[tauri::command]
pub(crate) fn save_profile(
    app: AppHandle,
    args: SaveProfileArgs,
) -> Result<OpResult, String> {
    match crate::profiles::save(&app, args.settings, &args.name) {
        Ok(()) => Ok(OpResult { ok: true, message: "档案已保存".into() }),
        Err(err) => Ok(OpResult { ok: false, message: err.to_string() }),
    }
}

#[tauri::command]
pub(crate) fn list_profiles(app: AppHandle) -> Result<Vec<ProfileMeta>, String> {
    crate::profiles::list(&app).map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn load_profile(app: AppHandle, name: String) -> Result<crate::profiles::ProfileFile, String> {
    crate::profiles::load(&app, &name).map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn delete_profile(app: AppHandle, name: String) -> Result<OpResult, String> {
    match crate::profiles::delete(&app, &name) {
        Ok(()) => Ok(OpResult { ok: true, message: "档案已删除".into() }),
        Err(err) => Ok(OpResult { ok: false, message: err.to_string() }),
    }
}

#[tauri::command]
pub(crate) fn capture_profile_settings(hal: State<'_, Hal>) -> Result<ProfileSettings, String> {
    crate::profiles::capture_current(&hal).map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn apply_profile(
    app: AppHandle,
    hal: State<'_, Hal>,
    name: String,
) -> Result<Vec<ApplyStep>, String> {
    crate::profiles::apply(&app, &hal, &name).map_err(|e| e.to_string())
}
