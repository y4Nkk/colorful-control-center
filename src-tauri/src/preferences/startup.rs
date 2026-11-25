use super::{Controller, desktop, placement};
use anyhow::Result;
use tauri::{AppHandle, Manager, utils::config::WindowConfig};

pub fn prepare_windows(context: &mut tauri::Context<tauri::Wry>, main_class: &str) -> Vec<WindowConfig> {
    context
        .config_mut()
        .app
        .windows
        .iter_mut()
        .filter_map(|window| {
            if !window.create {
                return None;
            }
            let mut config = window.clone();
            if config.label == "main" {
                config.window_classname = Some(main_class.into());
            }
            window.create = false;
            Some(config)
        })
        .collect()
}

pub fn create_window(app: &AppHandle, windows: &[WindowConfig]) -> Result<()> {
    let controller = app.state::<Controller>();
    let settings = controller.snapshot();
    for config in windows {
        let config = configure(config, controller.acceleration);
        let window = tauri::WebviewWindowBuilder::from_config(app, &config)?.build()?;
        if config.label == "main"
            && let Err(error) = placement::bind(&window, &config)
        {
            log::warn!("[window] 初始化主窗口布局失败：{error:#}");
        }
    }
    if let Ok(snapshot) = &settings {
        if let Err(error) = desktop::apply(app, &snapshot.settings) {
            controller.report(Some(format!("启动桌面偏好应用失败：{error:#}")));
        }
    }
    let failed = controller
        .snapshot()
        .map_or(true, |snapshot| snapshot.desktop_error.is_some());
    if failed || settings.is_ok_and(|snapshot| !snapshot.settings.start_minimized) {
        if let Some(window) = app.get_webview_window("main") {
            window.show()?;
        }
    }
    Ok(())
}

pub(super) fn configure(config: &WindowConfig, acceleration: bool) -> WindowConfig {
    let mut config = config.clone();
    config.visible = false;
    if !acceleration {
        let args = config.additional_browser_args.get_or_insert_with(|| {
            "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection".into()
        });
        if !args.split_ascii_whitespace().any(|v| v == "--disable-gpu") {
            args.push_str(" --disable-gpu");
        }
    }
    config
}
