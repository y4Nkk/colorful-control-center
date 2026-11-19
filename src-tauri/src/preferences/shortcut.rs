use anyhow::{Result, anyhow, bail};
use std::sync::{
    Mutex,
    atomic::{AtomicBool, AtomicU32, Ordering},
};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcut, Modifiers, Shortcut, ShortcutState};

#[derive(Default)]
pub(super) struct Active {
    pub key: Option<Shortcut>,
    pub cleanup: Option<Shortcut>,
}
#[derive(Default)]
pub struct Bindings {
    active: Mutex<Active>,
    id: AtomicU32,
    recording: AtomicBool,
}

pub fn parse(value: &str) -> Result<Option<Shortcut>> {
    if value.is_empty() {
        return Ok(None);
    }
    if value.len() > 64 {
        bail!("快捷键格式无效");
    }
    let key: Shortcut = value.parse().map_err(|e| anyhow!("快捷键格式无效：{e}"))?;
    let code = key.key.to_string();
    let supported = code == "Space"
        || code
            .strip_prefix("Key")
            .is_some_and(|s| s.len() == 1 && s.as_bytes()[0].is_ascii_uppercase())
        || code
            .strip_prefix("Digit")
            .is_some_and(|s| s.len() == 1 && s.as_bytes()[0].is_ascii_digit())
        || code
            .strip_prefix('F')
            .and_then(|s| s.parse::<u8>().ok())
            .is_some_and(|v| (1..=12).contains(&v));
    if !supported
        || !key
            .mods
            .intersects(Modifiers::CONTROL | Modifiers::ALT | Modifiers::SUPER)
    {
        bail!("请组合 Ctrl、Alt 或 Win 与字母、数字、空格或 F1–F12，可同时使用 Shift");
    }
    if key.to_string() != value {
        bail!("快捷键必须通过录入控件生成标准格式");
    }
    Ok(Some(key))
}

pub fn apply(app: &AppHandle, value: &str) -> Result<()> {
    let next = parse(value)?;
    let state = app.state::<Bindings>();
    let mut active = state
        .active
        .lock()
        .map_err(|_| anyhow!("快捷键状态不可用"))?;
    let manager = app.state::<GlobalShortcut<tauri::Wry>>();
    replace(
        &mut active,
        next,
        |key| manager.register(key).map_err(Into::into),
        |key| manager.unregister(key).map_err(Into::into),
    )?;
    state
        .id
        .store(next.map(|key| key.id()).unwrap_or(0), Ordering::Release);
    Ok(())
}

pub(super) fn replace(
    active: &mut Active,
    next: Option<Shortcut>,
    register: impl Fn(Shortcut) -> Result<()>,
    unregister: impl Fn(Shortcut) -> Result<()>,
) -> Result<()> {
    if let Some(cleanup) = active.cleanup {
        unregister(cleanup)?;
        active.cleanup = None;
    }
    if active.key == next {
        return Ok(());
    }
    if let Some(key) = next {
        register(key).map_err(|e| anyhow!("无法注册快捷键，可能已被系统或其他应用占用：{e}"))?;
    }
    if let Some(previous) = active.key {
        if let Err(error) = unregister(previous) {
            let mut message = format!("无法释放原快捷键：{error}");
            if let Some(key) = next {
                if let Err(cleanup) = unregister(key) {
                    active.cleanup = Some(key);
                    message.push_str(&format!("；清理新快捷键失败：{cleanup}"));
                }
            }
            bail!(message);
        }
    }
    active.key = next;
    Ok(())
}

pub fn recording(app: &AppHandle, enabled: bool) {
    app.state::<Bindings>()
        .recording
        .store(enabled, Ordering::Release);
}

pub fn plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, key, event| {
            let state = app.state::<Bindings>();
            if crate::shutdown::closing()
                || event.state() != ShortcutState::Pressed
                || state.recording.load(Ordering::Acquire)
                || state.id.load(Ordering::Acquire) != key.id()
            {
                return;
            }
            if let Err(error) = toggle(app) {
                let message = format!("全局快捷键执行失败：{error:#}");
                log::error!("{message}");
                app.state::<super::Controller>()
                    .report(Some(message.clone()));
                if let Err(error) = app.emit("preferences-error", message) {
                    log::error!("无法报告快捷键错误：{error}");
                }
            }
        })
        .build()
}

fn toggle(app: &AppHandle) -> Result<()> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| anyhow!("主窗口不可用"))?;
    if window.is_visible()? && !window.is_minimized()? && window.is_focused()? {
        window.hide()?;
    } else {
        window.show()?;
        window.unminimize()?;
        window.set_focus()?;
    }
    Ok(())
}
