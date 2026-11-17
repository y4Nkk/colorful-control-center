use super::{AppHandle, Manager, Record, Result, actions, cancelled, keymap, mpsc, osd, store};
use anyhow::bail;
use std::{cell::Cell, path::Path};
pub(super) fn execute(
    app: &AppHandle,
    path: &Path,
    approved: &Record,
    stopping: &mpsc::Receiver<()>,
    identity: &str,
) -> (Result<actions::Completion>, bool) {
    let stopped = Cell::new(false);
    let result = app
        .state::<crate::airplane::Controller>()
        .toggle(&app.state::<crate::hal::Hal>(), identity, || {
            if cancelled(stopping) {
                stopped.set(true);
                bail!("飞行模式事件已取消");
            }
            if store::load(path)?.record != *approved {
                bail!("飞行模式接管许可已撤销");
            }
            if app.state::<keymap::Controller>().recording_active()? {
                bail!("按键录制中，飞行模式事件已取消");
            }
            crate::hal::oem_owner::ensure_owner_absent()
        })
        .map(|enabled| actions::Completion {
            message: Some(osd::Message::result(
                "飞行模式",
                if enabled { "已开启" } else { "已关闭" },
            )),
            show_osd: true,
        });
    (result, stopped.get())
}
