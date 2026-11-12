//! 设置命令层（Phase 4）：开机自启（HKCU Run 注册表）+ 档案导入导出。
//! 自启指向当前 exe 路径；导入/导出复用 profiles 模块的 JSON 契约。

use serde::Serialize;
use tauri::AppHandle;
use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
use windows_sys::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
    HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ,
};

const RUN_KEY: &[u16] = &[
    0x53, 0x00, 0x6F, 0x00, 0x66, 0x00, 0x74, 0x00, 0x77, 0x00, 0x61, 0x00, 0x72, 0x00, 0x65,
    0x00, 0x5C, 0x00, 0x4D, 0x00, 0x69, 0x00, 0x63, 0x00, 0x72, 0x00, 0x6F, 0x00, 0x73, 0x00,
    0x6F, 0x00, 0x66, 0x00, 0x74, 0x00, 0x5C, 0x00, 0x57, 0x00, 0x69, 0x00, 0x6E, 0x00, 0x64,
    0x00, 0x6F, 0x00, 0x77, 0x00, 0x73, 0x00, 0x5C, 0x00, 0x43, 0x00, 0x75, 0x00, 0x72, 0x00,
    0x72, 0x00, 0x65, 0x00, 0x6E, 0x00, 0x74, 0x00, 0x56, 0x00, 0x65, 0x00, 0x72, 0x00, 0x73,
    0x00, 0x69, 0x00, 0x6F, 0x00, 0x6E, 0x00, 0x5C, 0x00, 0x52, 0x00, 0x75, 0x00, 0x6E, 0x00,
    0x00,
]; // "Software\Microsoft\Windows\CurrentVersion\Run"

fn value_name() -> Vec<u16> {
    let mut wide: Vec<u16> = "ColorfulControlCenter".encode_utf16().collect();
    wide.push(0);
    wide
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpResult {
    pub ok: bool,
    pub message: String,
}

#[tauri::command]
pub(crate) fn get_autostart() -> Result<bool, String> {
    unsafe {
        let mut key: HKEY = std::ptr::null_mut();
        let status = RegOpenKeyExW(HKEY_CURRENT_USER, RUN_KEY.as_ptr(), 0, KEY_QUERY_VALUE, &mut key);
        if status != ERROR_SUCCESS {
            return Ok(false);
        }
        let name = value_name();
        let mut data_type: u32 = 0;
        let mut needed: u32 = 0;
        let status = RegQueryValueExW(key, name.as_ptr(), std::ptr::null(), &mut data_type, std::ptr::null_mut(), &mut needed);
        RegCloseKey(key);
        Ok(status == ERROR_SUCCESS)
    }
}

#[tauri::command]
pub(crate) fn set_autostart(enable: bool) -> Result<OpResult, String> {
    unsafe {
        let mut key: HKEY = std::ptr::null_mut();
        let status = RegCreateKeyExW(
            HKEY_CURRENT_USER,
            RUN_KEY.as_ptr(),
            0,
            std::ptr::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE | KEY_QUERY_VALUE,
            std::ptr::null(),
            &mut key,
            std::ptr::null_mut(),
        );
        if status != ERROR_SUCCESS {
            return Ok(OpResult { ok: false, message: format!("打开注册表失败：{status:#x}") });
        }
        let name = value_name();
        let status = if enable {
            let exe = match std::env::current_exe() {
                Ok(path) => path,
                Err(err) => {
                    RegCloseKey(key);
                    return Ok(OpResult { ok: false, message: format!("无法定位当前程序：{err}") });
                }
            };
            let mut value: Vec<u16> = format!("\"{}\"", exe.display()).encode_utf16().collect();
            value.push(0);
            RegSetValueExW(
                key,
                name.as_ptr(),
                0,
                REG_SZ,
                value.as_ptr() as *const _,
                (value.len() * 2) as u32,
            )
        } else {
            RegDeleteValueW(key, name.as_ptr())
        };
        RegCloseKey(key);
        if status == ERROR_SUCCESS || (!enable && status == ERROR_FILE_NOT_FOUND) {
            Ok(OpResult {
                ok: true,
                message: if enable { "已开启开机自启" } else { "已关闭开机自启" }.into(),
            })
        } else {
            Ok(OpResult { ok: false, message: format!("注册表写入失败：{status:#x}") })
        }
    }
}

/// 导出档案为 JSON 文本（前端负责落盘下载）。
#[tauri::command]
pub(crate) fn export_profile(app: AppHandle, name: String) -> Result<String, String> {
    let file = crate::profiles::load(&app, &name).map_err(|e| e.to_string())?;
    serde_json::to_string_pretty(&file).map_err(|e| e.to_string())
}

/// 导入档案 JSON 文本（名称取自档案内容，冲突覆盖）。
#[tauri::command]
pub(crate) fn import_profile(app: AppHandle, json: String) -> Result<OpResult, String> {
    let file: crate::profiles::ProfileFile =
        serde_json::from_str(&json).map_err(|e| format!("JSON 解析失败：{e}"))?;
    let settings = file.settings;
    match crate::profiles::save(&app, settings, &file.name) {
        Ok(()) => Ok(OpResult { ok: true, message: format!("已导入：{}", file.name) }),
        Err(err) => Ok(OpResult { ok: false, message: err.to_string() }),
    }
}
