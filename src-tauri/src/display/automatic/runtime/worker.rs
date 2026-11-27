use super::super::{
    controller::{Shared, permit_recovery},
    engine, facts,
    fields::FileStore,
    native::Native,
    scheduler::Work,
};
use crate::{
    display,
    hal::{Hal, power_events::Source},
};
use anyhow::{Result, anyhow, bail};
use tauri::{Emitter, Manager};

pub(super) fn tick(shared: &Shared, store: &FileStore, app: &tauri::AppHandle) -> Result<()> {
    {
        let runtime = shared.lock()?;
        if !runtime.scheduler.polling_enabled() && runtime.trial.is_none() {
            return Ok(());
        }
    }
    let profile_busy = crate::startup::profile_in_progress(app)?;
    let hal = app.state::<Hal>();
    let display = app.state::<display::Controller>();
    let gate = hal.begin_write()?;
    let io = Native {
        hal: &hal,
        display: &display,
    };
    let should_poll = shared.lock()?.scheduler.polling_enabled();
    let evidence = should_poll
        .then(|| facts::read_evidence(&io))
        .transpose()?
        .map(|(evidence, _)| evidence);
    let mut waiting_for_mode = false;
    if let Some(evidence) = &evidence {
        waiting_for_mode = evidence.supply_mode_switch()
            && app
                .state::<crate::powerctl::mode::supply::Controller>()
                .waiting_for_mode(evidence.on_ac)?;
        let supply = match evidence.on_ac {
            Some(true) => Source::Ac,
            Some(false) => Source::Battery,
            None => Source::Unknown,
        };
        let mut runtime = shared.lock()?;
        runtime.scheduler.read_supply(supply, display::now_ms())?;
        if !waiting_for_mode {
            runtime.scheduler.mode(evidence.mode, display::now_ms())?;
        }
    }
    if handle_trial(shared, store, &io, &display, &hal)? {
        return Ok(());
    }
    let Some(evidence) = evidence else {
        return Ok(());
    };
    let can_work = shared.lock()?.scheduler.polling_enabled();
    if profile_busy || waiting_for_mode || !can_work || display.trial_pending()? {
        return Ok(());
    }
    let work = shared.lock()?.scheduler.take(display::now_ms());
    let Some(work) = work else {
        engine::active_record(store, evidence.fields)?;
        return Ok(());
    };
    let preview = match engine::preview(&io, store, work.trigger, work.previous_mode) {
        Ok(preview) => preview,
        Err(_) if shared.permit(work.epoch).is_err() => return Ok(()),
        Err(error) => return Err(error),
    };
    drop(gate);
    if preview
        .as_ref()
        .is_some_and(|transition| !transition.before.same_state(&transition.target))
    {
        reveal(app)?;
    }
    execute(shared, store, app, work)
}

fn handle_trial(
    shared: &Shared,
    store: &FileStore,
    io: &Native<'_>,
    display: &display::Controller,
    hal: &Hal,
) -> Result<bool> {
    let Some((id, epoch)) = shared.lock()?.trial else {
        return Ok(false);
    };
    if shared.permit(epoch).is_err() && display.automatic_trial_id()? == Some(id) {
        display.cancel(id)?;
    }
    if display.automatic_outcome(id)?.is_some() {
        engine::settle(io, store, display, id, &mut || permit_recovery(hal))?;
        shared.lock()?.trial = None;
        let enabled = shared.lock()?.scheduler.enabled;
        if enabled {
            shared.issue(None);
        }
    }
    Ok(true)
}

fn execute(shared: &Shared, store: &FileStore, app: &tauri::AppHandle, work: Work) -> Result<()> {
    let hal = app.state::<Hal>();
    let display = app.state::<display::Controller>();
    let _gate = hal.begin_write()?;
    let io = Native {
        hal: &hal,
        display: &display,
    };
    let mut permit = || {
        shared.permit(work.epoch)?;
        permit_recovery(&hal)
    };
    let result = engine::execute(
        &io,
        store,
        &display,
        work.trigger,
        work.previous_mode,
        &mut permit,
        &mut || permit_recovery(&hal),
    );
    match result {
        Ok(Some(id)) => {
            shared.lock()?.scheduler.acknowledge(work);
            shared.lock()?.trial = Some((id, work.epoch));
            shared.issue(None);
            if let Err(error) = app.emit("automatic-display-trial", ()) {
                log::warn!("[auto-refresh] 试用通知：{error}");
            }
        }
        Ok(None) => {
            shared.lock()?.scheduler.acknowledge(work);
            shared.issue(None);
        }
        Err(error) => {
            if let Some(id) = display.automatic_trial_id()? {
                shared.lock()?.trial = Some((id, work.epoch));
            } else if shared.permit(work.epoch).is_err()
                && store
                    .load()?
                    .is_some_and(|record| !record.interrupted() && record.transition().is_none())
            {
                return Ok(());
            }
            return Err(error);
        }
    }
    Ok(())
}

fn reveal(app: &tauri::AppHandle) -> Result<()> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| anyhow!("主窗口不可用"))?;
    window.show()?;
    window.unminimize()?;
    window.set_focus()?;
    if !window.is_visible()? {
        bail!("主窗口不可见，未开始自动显示试用");
    }
    Ok(())
}

pub(super) fn pause(shared: &Shared, issue: String) -> Result<bool> {
    let mut runtime = shared.lock()?;
    let changed = runtime.issue.as_deref() != Some(issue.as_str());
    if runtime.scheduler.enabled {
        runtime.scheduler.disable()?;
    }
    runtime.issue = Some(issue);
    shared.wake.notify_all();
    Ok(changed)
}

pub(super) fn cancel_on_exit(
    shared: &Shared,
    store: &FileStore,
    app: &tauri::AppHandle,
) -> Result<()> {
    let Some((id, _)) = shared.lock()?.trial else {
        return Ok(());
    };
    let hal = app.state::<Hal>();
    let display = app.state::<display::Controller>();
    let _gate = hal.begin_write()?;
    if display.automatic_trial_id()? == Some(id) {
        display.cancel(id)?;
    }
    let io = Native {
        hal: &hal,
        display: &display,
    };
    if display.automatic_outcome(id)?.is_some() {
        engine::settle(&io, store, &display, id, &mut || permit_recovery(&hal))?;
        shared.lock()?.trial = None;
    }
    Ok(())
}
