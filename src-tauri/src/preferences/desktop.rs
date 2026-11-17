use super::{
    model::{Preferences, Theme},
    shortcut,
};
use anyhow::{Context, Result, anyhow};
use tauri::{AppHandle, Manager};

pub fn apply(app: &AppHandle, settings: &Preferences) -> Result<()> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| anyhow!("主窗口不可用"))?;
    window
        .set_zoom(f64::from(settings.interface_scale) / 100.0)
        .context("应用界面缩放失败")?;
    window
        .set_always_on_top(settings.always_on_top)
        .context("应用窗口置顶失败")?;
    window
        .set_theme(match settings.theme {
            Theme::System => None,
            Theme::Light => Some(tauri::Theme::Light),
            Theme::Dark => Some(tauri::Theme::Dark),
        })
        .context("应用窗口主题失败")?;
    shortcut::apply(app, &settings.global_shortcut)?;
    log::set_max_level(settings.runtime_log_level.filter());
    Ok(())
}
