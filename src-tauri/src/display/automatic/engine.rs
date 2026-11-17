use super::{
    facts::{self, Facts, Source},
    fields::{self, FileStore, Io, Record, Transition},
};
use crate::display::{self, AutomaticResult, Controller, guardian};
use anyhow::{Result, anyhow, bail};
use ccc_core::oem_refresh::{self, Decision, Trigger};

pub(super) fn execute(
    io: &(impl Source + Io),
    store: &FileStore,
    display: &Controller,
    trigger: Trigger,
    previous_mode: Option<u8>,
    permit: &mut impl FnMut() -> Result<()>,
    recovery_permit: &mut impl FnMut() -> Result<()>,
) -> Result<Option<u64>> {
    let (facts, mut record, transition) = prepare(io, store, trigger, previous_mode)?;
    fields::verify(io, store, &record, permit)?;
    let Some(transition) = transition else {
        return Ok(None);
    };
    if display.trial_pending()? {
        bail!("已有显示试用，暂不执行自动刷新率");
    }
    fields::begin_transition(io, store, &mut record, transition.clone(), permit)?;
    let result = (|| {
        fields::change(io, store, &mut record, 2, transition.target_cache, permit)?;
        recheck(io, &facts, &record)?;
        fields::verify(io, store, &record, permit)?;
        display.begin_automatic(
            &transition.before,
            transition.target.frequency,
            display::now_ms(),
        )
    })();
    match result {
        Ok(Some(id)) => Ok(Some(id)),
        Ok(None) => {
            fields::finish_transition(io, store, &mut record, true, permit)?;
            Ok(None)
        }
        Err(error) => {
            let recovery = if display.automatic_trial_id()?.is_some() {
                Err(anyhow!("显示试用恢复待处理，保留事务记录"))
            } else {
                rollback_cache(io, store, &mut record, recovery_permit)
            };
            Err(anyhow!(ccc_core::msr::with_recovery(
                format!("{error:#}"),
                recovery.map_err(|error| format!("{error:#}"))
            )))
        }
    }
}

pub(super) fn preview(
    io: &(impl Source + Io),
    store: &FileStore,
    trigger: Trigger,
    previous_mode: Option<u8>,
) -> Result<Option<Transition>> {
    Ok(prepare(io, store, trigger, previous_mode)?.2)
}

fn prepare(
    io: &(impl Source + Io),
    store: &FileStore,
    trigger: Trigger,
    previous_mode: Option<u8>,
) -> Result<(Facts, Record, Option<Transition>)> {
    let facts = facts::read(io)?;
    let record = store.load()?.ok_or_else(|| anyhow!("自动刷新率尚未启用"))?;
    if !record.active()
        || record.interrupted()
        || record.transition().is_some()
        || facts.identity != record.identity()
        || facts.fields != record.current()
    {
        bail!("自动刷新率来源或事务已变化");
    }
    if !facts
        .display
        .same_state(record.owned_display().unwrap_or(record.initial_display()))
    {
        bail!("主屏模式已被外部修改，停止自动刷新率");
    }
    if trigger == Trigger::SupplyChange
        && (matches!(facts.product, 20720 | 20724) || facts.supply_mode_switch)
    {
        return Ok((facts, record, None));
    }
    let decision = oem_refresh::decide(oem_refresh::Facts {
        customer: facts.customer,
        mode: facts.mode,
        previous_mode,
        on_ac: facts.on_ac,
        supported: true,
        checked: facts.fields[1] == 1,
        trigger,
    })
    .map_err(anyhow::Error::msg)?;
    let target = (decision != Decision::None).then(|| transition(&facts, decision));
    Ok((facts, record, target))
}

pub(super) fn transition(facts: &Facts, decision: Decision) -> Transition {
    let mut target = facts.display.clone();
    target.frequency = match decision {
        Decision::Low | Decision::SaveThenLow => facts.low,
        Decision::RestoreSaved => facts.saved_or_highest,
        Decision::None => facts.display.frequency,
    };
    let target_cache = match decision {
        Decision::SaveThenLow => oem_refresh::original_saved_byte(facts.display.frequency),
        Decision::RestoreSaved => 0,
        _ => facts.fields[2],
    };
    Transition {
        before: facts.display.clone(),
        target,
        mode: facts.mode,
        on_ac: facts.on_ac,
        previous_cache: facts.fields[2],
        target_cache,
    }
}

pub(super) fn keep(
    io: &(impl Source + Io),
    store: &FileStore,
    display: &Controller,
    id: u64,
    expected: &display::Mode,
    permit: &mut impl FnMut() -> Result<()>,
) -> Result<()> {
    let record = store
        .load()?
        .ok_or_else(|| anyhow!("自动刷新率恢复记录缺失"))?;
    if !record.active() {
        bail!("自动刷新率已停用，不能保留试用");
    }
    let transition = record
        .transition()
        .ok_or_else(|| anyhow!("自动刷新率事务已结束"))?;
    let facts = facts::read(io)?;
    if facts.identity != record.identity()
        || facts.mode != transition.mode
        || facts.on_ac != transition.on_ac
        || !facts.display.same_state(&transition.target)
        || !expected.same_state(&transition.target)
    {
        bail!("自动刷新率试用条件已变化，请恢复本次试用");
    }
    fields::confirm(io, store, &record, permit)?;
    permit()?;
    display.keep_automatic(id, expected, display::now_ms())
}

fn recheck(io: &(impl Source + Io), before: &Facts, record: &Record) -> Result<()> {
    let current = facts::read(io)?;
    if current.identity != before.identity
        || current.mode != before.mode
        || current.on_ac != before.on_ac
        || !current.display.same_state(&before.display)
        || current.fields != record.current()
    {
        bail!("自动刷新率的显示器、模式或供电预览已变化");
    }
    Ok(())
}

fn rollback_cache(
    io: &impl Io,
    store: &FileStore,
    record: &mut Record,
    permit: &mut impl FnMut() -> Result<()>,
) -> Result<()> {
    let previous = record
        .transition()
        .ok_or_else(|| anyhow!("事务已变化"))?
        .previous_cache;
    fields::rollback_attempt(io, store, record, permit)?;
    fields::change(io, store, record, 2, previous, permit)?;
    fields::finish_transition(io, store, record, false, permit)
}

pub(super) fn settle(
    io: &(impl Source + Io),
    store: &FileStore,
    display: &Controller,
    id: u64,
    permit: &mut impl FnMut() -> Result<()>,
) -> Result<bool> {
    let Some(outcome) = display.automatic_outcome(id)? else {
        return Ok(false);
    };
    let mut record = store
        .load()?
        .ok_or_else(|| anyhow!("自动刷新率恢复记录缺失"))?;
    if let Some(transition) = record.transition().cloned() {
        let facts = facts::read(io)?;
        if facts.identity != record.identity() {
            bail!("自动刷新率设备已变化，保留恢复记录");
        }
        let expected = match outcome.result {
            AutomaticResult::Kept => &transition.target,
            AutomaticResult::Restored => &transition.before,
        };
        if !facts.display.same_state(expected) {
            bail!("自动刷新率试用后的主屏模式已变化");
        }
        match outcome.result {
            AutomaticResult::Kept => {
                fields::finish_transition(io, store, &mut record, true, permit)?
            }
            AutomaticResult::Restored => rollback_cache(io, store, &mut record, permit)?,
        }
    }
    display.clear_automatic_outcome(outcome)?;
    Ok(true)
}

pub(super) fn stop_and_restore(
    io: &(impl Source + Io),
    store: &FileStore,
    display: &Controller,
    expected: &str,
    permit: &mut impl FnMut() -> Result<()>,
) -> Result<()> {
    let view = super::state::read(io, store)?;
    if view.revision != expected || !view.can_restore {
        bail!("自动刷新率恢复预览已变化或不可恢复");
    }
    let mut record = store
        .load()?
        .ok_or_else(|| anyhow!("没有自动刷新率恢复记录"))?;
    if record.active() {
        fields::set_active(store, &mut record, false)?;
    }
    if let Some(id) = display.automatic_trial_id()? {
        display.cancel(id)?;
    }
    if let Some(outcome) = display.automatic_result()? {
        settle(io, store, display, outcome.id, permit)?;
        record = store
            .load()?
            .ok_or_else(|| anyhow!("自动刷新率恢复记录缺失"))?;
    }
    fields::rollback_attempt(io, store, &mut record, permit)?;
    fields::confirm(io, store, &record, permit)?;
    if let Some(transition) = record.transition().cloned() {
        display.recover_automatic(&guardian::Change {
            before: transition.before,
            target: transition.target,
        })?;
        fields::finish_transition(io, store, &mut record, false, permit)?;
    }
    if let Some(owned) = record.owned_display().cloned() {
        display.recover_automatic(&guardian::Change {
            before: record.initial_display().clone(),
            target: owned,
        })?;
        fields::clear_owned_display(io, store, &mut record, permit)?;
    }
    fields::restore(io, store, &mut record, permit)
}
