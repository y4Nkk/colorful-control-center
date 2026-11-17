use super::{boost, links, selection};
use crate::{
    gpu_power::{mode as node, profile as power},
    gpuctl::turbo::mode::ClockOrigin,
};
use anyhow::{Context, Result, bail};
mod authorization;
pub(crate) use authorization::{ApprovalPreview, Grant, authorization};

pub(crate) fn checkpoint(
    context: &links::Context<'_>,
    wanted: u8,
    options: Option<&boost::Values>,
    explicit: Option<&power::Target>,
    needs_gpu: bool,
) -> Result<links::Plan> {
    let mut plan = selection::checkpoint(context, wanted, options)?;
    let gpu = match plan.target.gpu.as_mut().context("档案模式GPU预览缺失")? {
        links::Review::Ready(gpu) => gpu,
        links::Review::Unavailable(reason) => bail!("{reason}"),
    };
    gpu.stages = gpu
        .node
        .as_ref()
        .map(|before| power::Stages::build(before, Some(wanted), explicit, needs_gpu))
        .transpose()?;
    selection::bind_target(&mut plan)?;
    Ok(plan)
}
pub(crate) fn stages(plan: &links::Plan) -> Result<Option<&power::Stages>> {
    Ok(plan
        .target
        .gpu
        .as_ref()
        .context("档案模式GPU预览缺失")?
        .target()?
        .stages
        .as_ref())
}
pub(crate) fn rebase(context: &links::Context<'_>, initial: &links::Plan) -> Result<links::Plan> {
    let mut current = selection::checkpoint(
        context,
        initial.core.target.mode,
        initial.core.options.as_ref(),
    )?;
    let original_gpu = initial
        .target
        .gpu
        .as_ref()
        .context("档案模式原GPU预览缺失")?
        .target()?;
    let now = current
        .target
        .gpu
        .as_ref()
        .context("档案模式当前GPU预览缺失")?
        .target()?;
    if let Some(graph) = stages(initial)?.filter(|graph| graph.preparation.is_some()) {
        if now.node.as_ref() != Some(&graph.mode_before())
            || initial.core.before.stored() != current.core.before.stored()
            || initial.core.target != current.core.target
            || initial.core.options_revision != current.core.options_revision
            || original_gpu.on_ac != now.on_ac
            || original_gpu.clocks.policy != now.clocks.policy
            || original_gpu.clocks.candidate.decision != now.clocks.candidate.decision
            || original_gpu
                .clocks
                .driver
                .as_ref()
                .is_some_and(|driver| now.clocks.driver.as_ref() != Some(driver))
        {
            bail!("档案准备后的模式、设备或频率前态已变化，未执行模式目标");
        }
        if now.clocks.driver.is_some() && !graph.before.enabled {
            if original_gpu.clocks.deferred_reason.is_none() {
                bail!("档案GPU驱动未获得延后核对许可");
            }
            current.clock_origin = Some(ClockOrigin::AfterEnable);
        }
        current.initial_node = Some(node::State::from_device(&graph.before)?);
    } else {
        let mut rebound = current.target.clone();
        if let Some(links::Review::Ready(gpu)) = rebound.gpu.as_mut() {
            gpu.stages = original_gpu.stages.clone();
        }
        current.target = rebound;
        selection::bind_target(&mut current)?;
        if current.target != initial.target {
            bail!("档案模式完整预览已变化，请重新确认");
        }
    }
    current.target = initial.target.clone();
    Ok(current)
}
pub(crate) fn prepare(
    session: &links::Session<'_>,
    plan: &links::Plan,
    permit: &mut dyn FnMut() -> Result<()>,
) -> Result<()> {
    if let Some(request) = stages(plan)?.and_then(|graph| graph.preparation.as_ref()) {
        session.perform(
            &node::State::from_device(&request.expected)?,
            request.target,
            permit,
        )?;
    }
    Ok(())
}
pub(crate) fn finish(
    session: &links::Session<'_>,
    plan: &links::Plan,
    enabled: bool,
    permit: &mut dyn FnMut() -> Result<()>,
) -> Result<()> {
    if let Some(request) = stages(plan)?
        .and_then(|graph| graph.finish.as_ref())
        .filter(|request| request.target.enabled == enabled)
    {
        session.perform(
            &node::State::from_device(&request.expected)?,
            request.target,
            permit,
        )?;
    }
    Ok(())
}
