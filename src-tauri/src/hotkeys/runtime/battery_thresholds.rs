use super::{AppHandle, Manager, Record, Result, actions, cancelled, keymap, mpsc, osd, store};
use anyhow::bail;

pub(super) fn execute(
    app: &AppHandle,
    path: &std::path::Path,
    approved: &Record,
    stopping: &mpsc::Receiver<()>,
    identity: &str,
    event: u8,
) -> (Result<actions::Completion>, bool) {
    let stopped = std::cell::Cell::new(false);
    let permit = || -> Result<()> {
        if cancelled(stopping) {
            stopped.set(true);
            bail!("电池阈值监听已停止");
        }
        if store::load(path)?.record != *approved {
            bail!("电池阈值策略许可已撤销");
        }
        if app.state::<keymap::Controller>().recording_active()? {
            bail!("按键录制中，电池阈值执行已取消");
        }
        crate::hal::oem_owner::ensure_owner_absent()
    };
    let result = app
        .state::<crate::hotkeys::Controller>()
        .battery_thresholds
        .run(&app.state::<crate::hal::Hal>(), identity, event, permit)
        .map(|detail| actions::Completion {
            message: Some(osd::Message::result("原厂低电量阈值", detail)),
            show_osd: true,
        });
    if cancelled(stopping) {
        stopped.set(true);
    }
    (result, stopped.get())
}
