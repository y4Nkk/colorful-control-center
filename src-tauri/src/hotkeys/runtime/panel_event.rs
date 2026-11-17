use super::{
    AppHandle, Manager, Record, Result, Status, actions, cancelled, keymap, mpsc, osd, store,
};
use anyhow::{Context, bail};
use std::{
    cell::Cell,
    path::Path,
    sync::{Arc, Mutex},
};
pub(super) fn execute(
    app: &AppHandle,
    path: &Path,
    approved: &Record,
    stopping: &mpsc::Receiver<()>,
    identity: &str,
    startup: bool,
) -> (Result<actions::Completion>, bool) {
    let stopped = Cell::new(false);
    let result = crate::hotkeys::panel_event::execute(
        &app.state::<crate::hal::Hal>(),
        identity,
        startup,
        || {
            if cancelled(stopping) {
                stopped.set(true);
                bail!("面板事件已取消");
            }
            if store::load(path)?.record != *approved {
                bail!("面板事件许可已撤销");
            }
            if app.state::<keymap::Controller>().recording_active()? {
                bail!("按键录制中，面板事件已取消");
            }
            crate::hal::oem_owner::ensure_owner_absent()
        },
    )
    .map(|detail| actions::Completion {
        message: Some(osd::Message::result("面板维护", detail)),
        show_osd: false,
    });
    (result, stopped.get())
}
pub(super) fn startup(
    app: &AppHandle,
    path: &Path,
    approved: &Record,
    stopping: &mpsc::Receiver<()>,
    status: &Arc<Mutex<Status>>,
) -> Result<()> {
    use super::{Action, Scope};
    if !approved.settings.actions.contains(&Scope::PanelEvent) {
        return Ok(());
    }
    let facts = actions::facts(app, &[Scope::PanelEvent]);
    approved.verify_action(&facts, Scope::PanelEvent)?;
    let identity = facts
        .capabilities
        .iter()
        .find(|c| c.scope == Scope::PanelEvent)
        .and_then(|c| c.identity.as_deref())
        .context("面板启动修复来源缺失")?;
    let (result, _) = execute(app, path, approved, stopping, identity, true);
    if let Some((message, _)) = super::completed_message(result, Action::PanelEvent) {
        super::report(app, status, Action::PanelEvent, false, message);
    }
    Ok(())
}
