use super::{AppHandle, Manager, Record, Result, actions, cancelled, keymap, mpsc, osd, store};
use anyhow::bail;

pub(super) fn execute(
    app: &AppHandle,
    path: &std::path::Path,
    approved: &Record,
    stopping: &mpsc::Receiver<()>,
    identity: &str,
) -> (Result<actions::Completion>, bool) {
    let stopped = std::cell::Cell::new(false);
    let permit = || -> Result<()> {
        if cancelled(stopping) {
            stopped.set(true);
            bail!("PD事件监听已停止");
        }
        if store::load(path)?.record != *approved {
            bail!("PD事件策略许可已撤销");
        }
        if app.state::<keymap::Controller>().recording_active()? {
            bail!("按键录制中，PD事件已取消");
        }
        crate::hal::oem_owner::ensure_owner_absent()
    };
    let result = app
        .state::<crate::platform::Controller>()
        .run_pd_event(&app.state::<crate::hal::Hal>(), identity, permit)
        .map(|changed| actions::Completion {
            message: Some(osd::Message::result(
                "原厂 PD 档位",
                if changed {
                    "原厂事件目标已写入并回读"
                } else {
                    "当前已是原厂事件目标，未重复写入"
                },
            )),
            show_osd: true,
        });
    if cancelled(stopping) {
        stopped.set(true);
    }
    (result, stopped.get())
}
