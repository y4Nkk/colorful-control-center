//! 托盘只请求档案预览，硬件写入仍经应用内确认与同一事务入口。
use tauri::{
    Emitter, Manager, WindowEvent,
    menu::{Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::TrayIconBuilder,
};

fn menu(app: &tauri::AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let menu = Menu::new(app)?;
    menu.append(&MenuItem::with_id(
        app,
        "show",
        "显示控制台",
        true,
        None::<&str>,
    )?)?;
    let profiles = Submenu::new(app, "性能档案", true)?;
    match crate::profiles::list(app) {
        Ok(list) if !list.is_empty() => {
            for profile in list {
                profiles.append(&MenuItem::with_id(
                    app,
                    format!("profile:{}", profile.name),
                    &profile.name,
                    true,
                    None::<&str>,
                )?)?;
            }
        }
        Ok(_) => profiles.append(&MenuItem::with_id(
            app,
            "empty",
            "暂无档案",
            false,
            None::<&str>,
        )?)?,
        Err(error) => {
            log::error!("[tray] 档案列表：{error:#}");
            profiles.append(&MenuItem::with_id(
                app,
                "error",
                "档案读取失败",
                false,
                None::<&str>,
            )?)?;
        }
    }
    menu.append(&profiles)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?)?;
    Ok(menu)
}

pub fn refresh(app: &tauri::AppHandle) {
    if let Some(tray) = app.tray_by_id("main-tray") {
        if let Err(error) = menu(app).and_then(|menu| tray.set_menu(Some(menu))) {
            log::error!("[tray] 更新菜单失败：{error}");
        }
    }
}

pub fn setup(app: &tauri::AppHandle) -> tauri::Result<()> {
    TrayIconBuilder::with_id("main-tray")
        .icon(
            app.default_window_icon()
                .cloned()
                .ok_or_else(|| tauri::Error::AssetNotFound("icon".into()))?,
        )
        .tooltip("Colorful Control Center")
        .menu(&menu(app)?)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            let id = event.id.as_ref();
            if id == "quit" {
                app.exit(0);
            } else if id == "show" {
                show_main(app);
            } else if let Some(name) = id.strip_prefix("profile:") {
                show_main(app);
                let _ = app.emit("profile-preview-request", name);
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let tauri::tray::TrayIconEvent::Click {
                button: tauri::tray::MouseButton::Left,
                button_state: tauri::tray::MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        })
        .build(app)?;
    if let Some(window) = app.get_webview_window("main") {
        let close = window.clone();
        window.on_window_event(move |event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let app = close.app_handle();
                let preferences = app.state::<crate::preferences::Controller>().snapshot();
                if preferences.is_ok_and(|s| {
                    s.settings.close_behavior == crate::preferences::model::CloseBehavior::Exit
                }) {
                    app.exit(0);
                } else if let Err(error) = close.hide() {
                    log::error!("隐藏到托盘失败：{error}");
                }
            }
        });
    }
    Ok(())
}

pub(crate) fn show_main(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}
