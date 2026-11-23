use anyhow::{Result, anyhow};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use tauri::Manager;

#[derive(Clone, serde::Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub(crate) enum OemStartupNotice {
    Closed,
    Failed { detail: String },
}

pub(crate) fn close_oem_at_launch(app: &tauri::AppHandle) {
    let enabled = app
        .state::<crate::preferences::Controller>()
        .snapshot()
        .is_ok_and(|snapshot| snapshot.settings.close_oem_on_startup);
    let notice = if enabled {
        match crate::hal::oem_owner::close_at_startup() {
            Ok(false) => None,
            Ok(true) => {
                log::info!("[startup] 已结束原厂 FnKey");
                Some(OemStartupNotice::Closed)
            }
            Err(error) => {
                let detail = format!("{error:#}");
                log::warn!("[startup] {detail}");
                Some(OemStartupNotice::Failed { detail })
            }
        }
    } else {
        None
    };
    app.manage(notice);
}

#[derive(Default)]
pub struct LaunchReport {
    error: Mutex<Option<String>>,
    preparing: Arc<AtomicBool>,
}
struct Preparation(Arc<AtomicBool>);
impl Drop for Preparation {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}
impl LaunchReport {
    pub(super) fn last_error(&self) -> Result<Option<String>> {
        Ok(self
            .error
            .lock()
            .map_err(|_| anyhow!("启动结果不可用"))?
            .clone())
    }
    fn prepare(&self) -> Preparation {
        self.preparing.store(true, Ordering::Release);
        Preparation(self.preparing.clone())
    }
    fn record_error(&self, message: String) {
        if let Ok(mut error) = self.error.lock() {
            *error = Some(message.clone());
        }
        log::error!("[startup] {message}");
    }
}
pub(crate) fn profile_in_progress(app: &tauri::AppHandle) -> Result<bool> {
    if preparing(app) {
        return Ok(true);
    }
    app.state::<crate::profiles::application::Controller>()
        .is_busy(&app.state::<crate::display::Controller>())
}
pub(crate) fn preparing(app: &tauri::AppHandle) -> bool {
    app.state::<LaunchReport>()
        .preparing
        .load(Ordering::Acquire)
}
pub fn on_launch(app: &tauri::AppHandle) {
    if !std::env::args().any(|arg| arg == "--autostart") {
        return;
    }
    let Some(operation) = crate::shutdown::enter() else {
        return;
    };
    let preparation = app.state::<LaunchReport>().prepare();
    let handle = app.clone();
    let result = std::thread::Builder::new()
        .name("startup-profile".into())
        .spawn(move || {
            let _operation = operation;
            let _preparation = preparation;
            if let Err(error) = apply_selected(&handle) {
                handle
                    .state::<LaunchReport>()
                    .record_error(format!("启动档案失败：{error:#}"));
            }
        });
    if let Err(error) = result {
        app.state::<LaunchReport>()
            .record_error(format!("启动档案线程创建失败：{error:#}"));
    }
}
pub fn on_second_launch(app: &tauri::AppHandle, args: &[String]) {
    if !args.iter().any(|arg| arg == "--autostart") {
        crate::tray::show_main(app);
    }
}
fn apply_selected(app: &tauri::AppHandle) -> Result<()> {
    let selected = super::read(app)?;
    let Some(binding) = selected.profile.as_ref() else {
        return Ok(());
    };
    let hal = app.state::<crate::hal::Hal>();
    let gpu = app.state::<crate::gpuctl::GpuController>();
    let file = binding.load(&crate::profiles::profiles_dir(app)?)?;
    let permission = super::authorization::permission(app.path().app_data_dir()?, selected.clone());
    let result =
        crate::profiles::apply_startup(app, &hal, &gpu, &file, binding.mode.as_ref(), permission)?;
    if result.pending.is_some() {
        crate::tray::show_main(app);
    }
    crate::profiles::application::log_application(&result);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preparation_is_visible_before_dispatch_and_released_after_thread_completion() {
        let report = LaunchReport::default();
        let preparing = report.prepare();
        assert!(report.preparing.load(Ordering::Acquire));
        std::thread::spawn(move || {
            drop(preparing);
        })
        .join()
        .unwrap();
        assert!(!report.preparing.load(Ordering::Acquire));
    }
    #[test]
    fn abandoning_dispatch_and_unwinding_do_not_leave_startup_blocked() {
        let report = LaunchReport::default();
        let preparation = report.prepare();
        let abandoned = move || drop(preparation);
        drop(abandoned);
        assert!(!report.preparing.load(Ordering::Acquire));
        let preparation = report.prepare();
        let result = std::panic::catch_unwind(move || {
            let _preparation = preparation;
            panic!("隔离启动失败");
        });
        assert!(result.is_err());
        assert!(!report.preparing.load(Ordering::Acquire));
    }
}
