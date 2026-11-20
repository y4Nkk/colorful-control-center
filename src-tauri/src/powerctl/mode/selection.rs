use super::{ModeProfile, Target, boost, links, workflow};
use anyhow::{Result, bail};

pub(crate) fn read(context: &links::Context<'_>) -> Result<ModeProfile> {
    let mut profile = super::read(context.hal)?;
    for target in &mut profile.targets {
        match checkpoint(context, target.mode, None) {
            Ok(plan) => *target = plan.target,
            Err(error) => {
                target.gpu = Some(context.selection(target.mode));
                target.unavailable_reason = Some(format!("{error:#}"));
            }
        }
    }
    Ok(profile)
}
pub(crate) fn read_options(context: &links::Context<'_>) -> Result<boost::Profile> {
    boost::read(context.hal)
}
pub(crate) fn checkpoint(
    context: &links::Context<'_>,
    wanted: u8,
    options: Option<&boost::Values>,
) -> Result<links::Plan> {
    let core = workflow::checkpoint_for_profile(context.hal, wanted, options)?;
    let review = context.selection(wanted);
    let effects = review.target()?.effects()?;
    let mut plan = links::Plan::new(core, effects)?;
    plan.target.from_mode = Some(plan.core.before.core.mode);
    plan.target.gpu = Some(review);
    bind_target(&mut plan)?;
    Ok(plan)
}
pub(crate) fn bind_target(plan: &mut links::Plan) -> Result<()> {
    use sha2::{Digest, Sha256};
    plan.target.revision = Some(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&(
            "oem-mode-preview-1",
            plan.core.before.stored(),
            &plan.core.target,
            &plan.core.options_revision,
            &plan.target.gpu,
        ))?)
    ));
    Ok(())
}
pub(crate) fn apply_preview(
    context: links::Context<'_>,
    expected: &Target,
    permit: &mut dyn FnMut() -> Result<()>,
) -> Result<()> {
    apply_checkpoint(context, expected, &mut |_, _, _| Ok(()), permit)
}

pub(crate) fn checked_target(
    context: &links::Context<'_>,
    expected: &Target,
) -> Result<links::Plan> {
    let plan = checkpoint(context, expected.mode, None)?;
    if plan.target != *expected {
        bail!("模式联动条件已变化，请重新读取后再切换");
    }
    Ok(plan)
}

pub(crate) fn apply_checkpoint(
    context: links::Context<'_>,
    expected: &Target,
    frequency_checkpoint: &mut dyn FnMut(
        &crate::gpuctl::turbo::mode::Write,
        crate::gpuctl::turbo::mode::ClockOrigin,
        &str,
    ) -> Result<()>,
    permit: &mut dyn FnMut() -> Result<()>,
) -> Result<()> {
    let session = context.session()?;
    let gate = session.context.hal.begin_write()?;
    permit()?;
    let plan = checked_target(&session.context, expected)?;
    if let Some(action) = plan.effects.node_action {
        let current = expected
            .gpu
            .as_ref()
            .and_then(|review| review.target().ok())
            .and_then(|target| target.node.as_ref())
            .ok_or_else(|| anyhow::anyhow!("模式独显前态缺失"))?;
        session.freeze_node(current, action.target)?;
    }
    drop(gate);
    permit()?;
    session.authorize()?;
    let result = (|| {
        let _gate = session.context.hal.begin_write()?;
        permit()?;
        let current = checkpoint(&session.context, expected.mode, None)?;
        if current.target != *expected {
            bail!("授权期间模式联动条件已变化，请重新读取后再切换");
        }
        plan.execute(&session, frequency_checkpoint, permit)
    })();
    match (result, session.finish()) {
        (Err(error), Err(cleanup)) => bail!("{error:#}；模式授权收尾失败：{cleanup:#}"),
        (Err(error), _) | (_, Err(error)) => Err(error),
        _ => Ok(()),
    }
}
