mod model;
mod native;
use anyhow::{Result, anyhow};
pub use model::{Level, Message};
use serde::Serialize;
use std::{sync::Mutex, time::Instant};
use tauri::{AppHandle, Manager};

#[derive(Default)]
pub struct Controller {
    ready: std::sync::atomic::AtomicBool,
    state: Mutex<model::Model>,
    timer: Mutex<Option<native::Timer>>,
    clock: std::sync::OnceLock<Instant>,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    #[serde(flatten)]
    state: model::Snapshot,
    appearance: Option<crate::preferences::model::Appearance>,
    appearance_error: Option<String>,
}
impl Controller {
    pub(crate) fn error(&self) -> Result<Option<String>> {
        Ok(self
            .state
            .lock()
            .map_err(|_| anyhow!("屏显状态异常"))?
            .snapshot
            .error
            .clone())
    }
    pub fn snapshot(&self, app: &AppHandle) -> Result<Snapshot> {
        let appearance = app.state::<crate::preferences::Controller>().snapshot();
        Ok(Snapshot {
            state: self
                .state
                .lock()
                .map_err(|_| anyhow!("屏显状态异常"))?
                .snapshot
                .clone(),
            appearance_error: appearance.as_ref().err().map(|e| format!("{e:#}")),
            appearance: appearance.ok().map(|s| (&s.settings).into()),
        })
    }
    fn now(&self) -> std::time::Duration {
        self.clock.get_or_init(Instant::now).elapsed()
    }
    pub fn show(&self, app: &AppHandle, message: Message) -> Result<()> {
        let sequence = self
            .state
            .lock()
            .map_err(|_| anyhow!("屏显状态异常"))?
            .post(message)?;
        let handle = app.clone();
        app.run_on_main_thread(move || native::present(&handle, sequence))?;
        Ok(())
    }
    pub fn stop(&self) {
        if let Ok(mut timer) = self.timer.lock() {
            timer.take();
        }
    }
    pub fn preview(&self, app: &AppHandle) -> Result<()> {
        let mut message = Message::result("屏显预览", "示例亮度 · 不修改设备");
        message.level = Some(Level { value: 3, max: 4 });
        self.show(app, message)
    }
}
