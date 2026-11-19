mod controls;
mod native;

use crate::{preferences, shutdown};
use anyhow::Result;
use native::{Action, Tray};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, WindowEvent};

pub(crate) use controls::sample;

#[derive(Serialize, Clone)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum NavigationRequest {
    Profile { name: String },
    Cooling,
}

pub(crate) fn start(app: &AppHandle) -> Result<()> {
    let handle = app.clone();
    let dispatch = move |action| match action {
        Action::Quit => shutdown::request(),
        Action::Control(control) => controls::dispatch(&handle, control),
        action if !shutdown::closing() => {
            let app = handle.clone();
            let _ = handle.run_on_main_thread(move || {
                if shutdown::closing() {
                    return;
                }
                show_main(&app);
                let request = match action {
                    Action::Profile(name) => Some(NavigationRequest::Profile { name }),
                    Action::Cooling => Some(NavigationRequest::Cooling),
                    _ => None,
                };
                if let Some(request) = request
                    && let Err(error) = app.emit_to("main", "tray-navigation-request", request)
                {
                    log::error!("[tray] 打开设置：{error:#}");
                }
            });
        }
        _ => (),
    };
    app.manage(Tray::start(
        Some(tauri::utils::platform::WINDOWS_APP_ICON_RESOURCE_ID),
        dispatch,
        shutdown::closing,
    )?);
    Ok(())
}

pub fn refresh_profiles(app: &AppHandle) {
    if let Some(tray) = app.try_state::<Tray>() {
        tray.set_profiles(match crate::profiles::list(app) {
            Ok(list) => Some(list.into_iter().map(|profile| profile.name).collect()),
            Err(error) => {
                log::error!("[tray] 档案列表：{error:#}");
                None
            }
        });
    }
}

pub(crate) fn bind_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let close = window.clone();
        window.on_window_event(move |event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                if shutdown::closing() {
                    return;
                }
                let app = close.app_handle();
                let preferences = app.state::<preferences::Controller>().snapshot();
                if preferences.is_ok_and(|s| {
                    s.settings.close_behavior == preferences::model::CloseBehavior::Exit
                }) {
                    shutdown::request();
                } else if let Err(error) = close.hide() {
                    log::error!("[tray] 隐藏失败：{error}");
                }
            }
        });
    }
}

pub(crate) fn stop(app: &AppHandle) -> Result<()> {
    app.try_state::<Tray>().map_or(Ok(()), |tray| tray.stop())
}

pub(crate) fn show_main(app: &AppHandle) {
    if !shutdown::closing()
        && let Some(window) = app.get_webview_window("main")
    {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}
