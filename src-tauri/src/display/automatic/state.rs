use super::{
    facts::{self, Facts, Source},
    fields::{self, FileStore, Io, Record},
};
use anyhow::{Result, anyhow, bail};
use serde::Serialize;
use sha2::{Digest, Sha256};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct View {
    pub revision: String,
    pub current: Option<Facts>,
    pub initial: Option<[u8; 3]>,
    pub initial_display: Option<crate::display::Mode>,
    pub preview: Option<crate::display::guardian::Change>,
    pub active: bool,
    pub can_start: bool,
    pub can_restore: bool,
    pub error: Option<String>,
}
pub(super) fn read(io: &(impl Source + Io), store: &FileStore) -> Result<View> {
    let current = facts::read(io);
    let loaded = store.load();
    let error = match (&current, &loaded) {
        (Err(error), _) | (_, Err(error)) => Some(format!("{error:#}")),
        (Ok(facts), Ok(Some(record))) if facts.identity != record.identity() => {
            Some("主显示器或原厂能力已变化，保留恢复记录".into())
        }
        (Ok(_), Ok(Some(record))) if record.interrupted() => {
            Some("自动刷新率事务未完成，请先恢复".into())
        }
        (Ok(_), Ok(Some(record))) if record.transition().is_some() => {
            Some("自动刷新率显示事务未完成，请先恢复".into())
        }
        (Ok(facts), Ok(Some(record))) if facts.fields != record.current() => {
            Some("原厂刷新率字段已被外部修改".into())
        }
        (Ok(facts), Ok(Some(record)))
            if record.active()
                && !facts
                    .display
                    .same_state(record.owned_display().unwrap_or(record.initial_display())) =>
        {
            Some("主屏模式已被外部修改，自动刷新率已暂停".into())
        }
        _ => None,
    };
    let facts = current.ok();
    let record = loaded.ok().flatten();
    let preview = if record.is_none() {
        facts.as_ref().and_then(|facts| {
            let decision = ccc_core::oem_refresh::decide(ccc_core::oem_refresh::Facts {
                customer: facts.customer,
                mode: facts.mode,
                previous_mode: None,
                on_ac: facts.on_ac,
                supported: true,
                checked: true,
                trigger: ccc_core::oem_refresh::Trigger::UserChange,
            })
            .ok()?;
            if decision == ccc_core::oem_refresh::Decision::None {
                return None;
            }
            let transition = super::engine::transition(facts, decision);
            Some(crate::display::guardian::Change {
                before: transition.before,
                target: transition.target,
            })
        })
    } else {
        None
    };
    let revision = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&(&facts, &record, &error))?)
    );
    Ok(View {
        active: record.as_ref().is_some_and(Record::active) && error.is_none(),
        can_start: record.is_none()
            && error.is_none()
            && facts
                .as_ref()
                .is_some_and(|facts| facts.default_enabled && facts.on_ac.is_some()),
        can_restore: record
            .as_ref()
            .zip(facts.as_ref())
            .is_some_and(|(record, facts)| record.identity() == facts.identity),
        initial: record.as_ref().map(Record::initial),
        initial_display: record
            .as_ref()
            .map(|record| record.initial_display().clone()),
        preview,
        current: facts,
        revision,
        error,
    })
}
pub(super) fn start(
    io: &(impl Source + Io),
    store: &FileStore,
    expected: &str,
    permit: &mut impl FnMut() -> Result<()>,
    recovery_permit: &mut impl FnMut() -> Result<()>,
) -> Result<()> {
    let view = read(io, store)?;
    if view.revision != expected || !view.can_start {
        bail!("自动刷新率预览已变化或不可接管");
    }
    let facts = view
        .current
        .ok_or_else(|| anyhow!("自动刷新率事实不可用"))?;
    permit()?;
    let mut record = Record::new(facts.identity, facts.fields, facts.display)?;
    store.save(None, &record)?;
    let result = (|| {
        fields::change(io, store, &mut record, 0, 1, permit)?;
        fields::change(io, store, &mut record, 1, 1, permit)?;
        fields::confirm(io, store, &record, permit)?;
        fields::set_active(store, &mut record, true)
    })();
    if let Err(error) = result {
        let recovery = fields::restore(io, store, &mut record, recovery_permit);
        return Err(anyhow!(ccc_core::msr::with_recovery(
            format!("{error:#}"),
            recovery.map_err(|error| format!("{error:#}"))
        )));
    }
    Ok(())
}
