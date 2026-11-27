use crate::{assembly, preferences, shutdown, tray};
use anyhow::{Context, Result};
use serde::Serialize;
use std::{
    sync::Mutex,
    thread::{self, JoinHandle},
};
use tauri::{AppHandle, Manager};

#[derive(Clone, Default, Serialize)]
#[serde(tag = "stage", rename_all = "camelCase")]
enum Stage {
    #[default]
    CheckingOem,
    Hardware,
    Services,
    Ready,
    Failed {
        detail: String,
    },
}

#[derive(Clone, Default, Serialize)]
pub(crate) struct Snapshot {
    #[serde(flatten)]
    stage: Stage,
    appearance: Option<preferences::model::Appearance>,
}

#[derive(Default)]
pub(crate) struct Preparation {
    state: Mutex<Snapshot>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl Preparation {
    pub(crate) fn set_appearance(&self, snapshot: Option<preferences::Snapshot>) {
        self.state.lock().unwrap().appearance =
            snapshot.as_ref().map(|value| (&value.settings).into());
    }

    pub(crate) fn snapshot(&self) -> Snapshot {
        self.state.lock().unwrap().clone()
    }

    fn stage(&self, stage: Stage) {
        self.state.lock().unwrap().stage = stage;
    }

    pub(crate) fn start(&self, app: &AppHandle, login: bool) {
        let mut worker = self.worker.lock().unwrap();
        if shutdown::closing() {
            return;
        }
        let handle = app.clone();
        match thread::Builder::new()
            .name("application-preparation".into())
            .spawn(move || run(&handle, login))
        {
            Ok(thread) => *worker = Some(thread),
            Err(error) => fail(app, format!("无法启动初始化线程：{error}")),
        }
    }
}

pub(crate) fn ready(app: &AppHandle) -> bool {
    matches!(
        app.state::<Preparation>().state.lock().unwrap().stage,
        Stage::Ready
    )
}

pub(crate) fn wait_for_preparation(app: &AppHandle) -> Result<()> {
    let preparation = app.state::<Preparation>();
    let worker = preparation.worker.lock().unwrap().take();
    if let Some(worker) = worker {
        worker
            .join()
            .map_err(|_| anyhow::anyhow!("应用初始化线程异常退出"))?;
    }
    Ok(())
}

fn run(app: &AppHandle, login: bool) {
    let Some(_operation) = shutdown::enter() else {
        return;
    };
    let state = app.state::<Preparation>();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<()> {
        super::close_oem_at_launch(app);
        if shutdown::closing() {
            return Ok(());
        }
        state.stage(Stage::Hardware);
        assembly::setup(app).context("初始化硬件控制失败")?;
        if shutdown::closing() {
            return Ok(());
        }
        state.stage(Stage::Services);
        if login {
            super::apply_login_profile(app);
        }
        crate::start_workers(app);
        state.stage(Stage::Ready);
        Ok(())
    }));
    match result {
        Ok(Ok(())) => {}
        Ok(Err(error)) => fail(app, format!("{error:#}")),
        Err(cause) => {
            let detail = cause
                .downcast_ref::<String>()
                .map(String::as_str)
                .or_else(|| cause.downcast_ref::<&str>().copied())
                .unwrap_or("未知原因");
            fail(app, format!("应用初始化线程异常退出：{detail}"));
        }
    }
}

fn fail(app: &AppHandle, detail: String) {
    log::error!("[startup] {detail}");
    app.state::<Preparation>().stage(Stage::Failed { detail });
    let handle = app.clone();
    if let Err(error) = app.run_on_main_thread(move || tray::show_main(&handle)) {
        log::error!("[startup] 显示初始化失败详情：{error}");
    }
}
