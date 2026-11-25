use crate::{hal::oem_owner, preferences, shutdown};
use std::{thread, time::Duration};
use tauri::{AppHandle, Emitter, Manager};

#[derive(Clone, serde::Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub(crate) enum OemStartupNotice {
    Closed {
        executables: Vec<&'static str>,
    },
    Failed {
        detail: String,
        closed: Vec<&'static str>,
    },
}

impl OemStartupNotice {
    fn closed(&self) -> &[&'static str] {
        match self {
            Self::Closed { executables } => executables,
            Self::Failed { closed, .. } => closed,
        }
    }
}

pub(crate) fn close_oem_at_launch(app: &AppHandle) {
    let notice = check(app);
    log_result(notice.as_ref(), &mut None);
    app.manage(notice);
}

pub(crate) fn start_oem_worker(app: &AppHandle) {
    let app = app.clone();
    let worker = thread::Builder::new()
        .name("oem-auto-close".into())
        .spawn(move || {
            let mut last_failure =
                failure(app.state::<Option<OemStartupNotice>>().inner().as_ref())
                    .map(str::to_owned);
            loop {
                thread::sleep(Duration::from_secs(3));
                let Some(_operation) = shutdown::enter() else {
                    break;
                };
                let notice = check(&app);
                log_result(notice.as_ref(), &mut last_failure);
                if notice
                    .as_ref()
                    .is_some_and(|notice| !notice.closed().is_empty())
                    && let Err(error) = app.emit_to("main", "hardware-controls-changed", ())
                {
                    log::error!("[oem-auto-close] 刷新控制状态：{error:#}");
                }
            }
        });
    if let Err(error) = worker {
        log::error!("[oem-auto-close] 无法启动运行检查：{error}");
    }
}

fn check(app: &AppHandle) -> Option<OemStartupNotice> {
    let result = app
        .state::<preferences::Controller>()
        .with_oem_auto_close(oem_owner::close_running);
    match result {
        Ok(Some(report)) if !report.failures.is_empty() => Some(OemStartupNotice::Failed {
            detail: report.failures.join("；"),
            closed: report.closed,
        }),
        Ok(Some(report)) if !report.closed.is_empty() => Some(OemStartupNotice::Closed {
            executables: report.closed,
        }),
        Ok(_) => None,
        Err(error) => Some(OemStartupNotice::Failed {
            detail: format!("{error:#}"),
            closed: Vec::new(),
        }),
    }
}

fn failure(notice: Option<&OemStartupNotice>) -> Option<&str> {
    match notice {
        Some(OemStartupNotice::Failed { detail, .. }) => Some(detail),
        _ => None,
    }
}

fn log_result(notice: Option<&OemStartupNotice>, last_failure: &mut Option<String>) {
    if let Some(closed) = notice
        .map(OemStartupNotice::closed)
        .filter(|closed| !closed.is_empty())
    {
        log::info!("[oem-auto-close] 已关闭 {}", closed.join("、"));
    }
    let next = failure(notice);
    if next != last_failure.as_deref() {
        if let Some(detail) = next {
            log::warn!("[oem-auto-close] {detail}");
        }
        *last_failure = next.map(str::to_owned);
    }
}
