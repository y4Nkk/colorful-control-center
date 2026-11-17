use super::*;
use model::{Baseline, Phase, Record, Target};

pub(crate) struct Plan {
    pub(super) before: Frame,
    pub(super) revision: String,
    pub(super) record: Record,
    pub(super) target: Target,
    before_configuration: Configuration,
    scope: Vec<Device>,
    flags: [u8; 2],
}
pub(super) struct Pending {
    loaded: store::Loaded,
    attempted: bool,
}
pub(super) fn prepare(frame: Frame, target: &Target) -> Result<Plan> {
    build(frame, target, false)
}
pub(super) fn initial(frame: Frame) -> Result<Plan> {
    let record = frame.loaded.record.as_ref().context("没有宏映射初始快照")?;
    let flags = record.baseline.flags;
    let target = Target {
        configuration: model::subset(&record.baseline.configuration, &record.owned)?,
        programs_enabled: true,
    };
    let mut plan = build(frame, &target, true)?;
    plan.flags = flags;
    Ok(plan)
}
fn build(frame: Frame, target: &Target, restoring: bool) -> Result<Plan> {
    target.validate()?;
    let revision = frame.revision()?;
    frame.review(&revision, restoring)?;
    let before_configuration = frame
        .current()
        .context("原厂活动配置不可读，无法建立恢复基线")?
        .clone();
    let baseline = frame
        .loaded
        .record
        .as_ref()
        .map(|r| r.baseline.clone())
        .or_else(|| {
            frame.facts.original.as_ref().map(|s| Baseline {
                name: s.name.clone(),
                configuration: s.configuration.clone(),
                flags: frame.facts.flags,
            })
        })
        .context("没有可读取的原厂配置恢复基线")?;
    let scope: Vec<_> = target
        .configuration
        .devices
        .iter()
        .map(|device| device.device)
        .collect();
    let mut owned = frame
        .loaded
        .record
        .as_ref()
        .map_or_else(Vec::new, |r| r.owned.clone());
    for device in &scope {
        if !owned.contains(device) {
            owned.push(*device);
        }
    }
    let record = Record {
        version: 1,
        baseline,
        current: model::merge(&before_configuration, &target.configuration)?,
        owned,
        phase: Phase::Applied,
    };
    record.validate()?;
    let flags = model::flags(frame.facts.flags, &target.configuration);
    Ok(Plan {
        before: frame,
        revision,
        record,
        target: target.clone(),
        before_configuration,
        scope,
        flags,
    })
}
pub(super) fn execute(
    io: &(impl Io + ?Sized),
    store: &impl store::Store,
    plan: &Plan,
    pending: &mut Option<Pending>,
) -> Result<()> {
    let mut record = plan.record.clone();
    record.phase = Phase::Prepared;
    let loaded = store.replace(&plan.before.loaded, &record)?;
    *pending = Some(Pending {
        loaded,
        attempted: false,
    });
    io.ensure_available().context("尚未写入过滤器")?;
    pending.as_mut().expect("prepared journal").attempted = true;
    io.replace(&plan.target.configuration)?;
    io.save_flags(plan.flags, &plan.scope)
}
pub(super) fn commit(
    io: &(impl Io + ?Sized),
    store: &impl store::Store,
    plan: &Plan,
    pending: &mut Pending,
) -> Result<()> {
    io.ensure_available()?;
    pending.loaded = store.replace(&pending.loaded, &plan.record)?;
    Ok(())
}
pub(super) fn rollback(
    io: &(impl Io + ?Sized),
    store: &impl store::Store,
    plan: &Plan,
    pending: &Pending,
) -> Result<()> {
    if store.load()?.revision != pending.loaded.revision {
        bail!("映射恢复记录已被外部修改，未向过滤器回放旧快照");
    }
    let facts = io.facts()?;
    if facts.capability != plan.before.facts.capability
        || facts.original.as_ref().map(|s| &s.fingerprint)
            != plan.before.facts.original.as_ref().map(|s| &s.fingerprint)
        || facts.original_error != plan.before.facts.original_error
    {
        bail!("映射能力或来源已变化，恢复记录保留，未覆盖其他配置");
    }
    if pending.attempted {
        if plan
            .before
            .loaded
            .record
            .as_ref()
            .is_some_and(|r| r.phase == Phase::Prepared)
        {
            bail!("先前映射执行状态未知，未用保存意图伪造前态；请重试初始恢复");
        }
        compensate(
            io,
            &model::subset(&plan.before_configuration, &plan.scope)?,
            plan.before.facts.flags,
            &plan.scope,
        )?;
    }
    let before = plan.before.loaded.record.clone().unwrap_or_else(|| {
        let mut record = plan.record.clone();
        record.current = plan.before_configuration.clone();
        record
    });
    store.replace(&pending.loaded, &before)?;
    Ok(())
}
fn compensate(
    io: &(impl Io + ?Sized),
    configuration: &Configuration,
    flags: [u8; 2],
    scope: &[Device],
) -> Result<()> {
    io.ensure_available()?;
    let mapping = io.replace(configuration);
    let saved = io.save_flags(flags, scope);
    let errors = [mapping.err(), saved.err()]
        .into_iter()
        .flatten()
        .map(|e| format!("{e:#}"))
        .collect::<Vec<_>>();
    if !errors.is_empty() {
        bail!(errors.join("；"));
    }
    Ok(())
}
