use super::{Snapshot, energy, links, workflow};
use crate::{gpu_power::mode as node, gpuctl::turbo::mode::ClockObservation};
use anyhow::{Context as _, Result, bail};

pub(super) fn preview(
    context: &links::Context<'_>,
) -> Result<(Option<workflow::RestoreTarget>, Option<String>)> {
    let Some(mut target) = workflow::restore_target(context.hal)? else {
        return Ok((None, None));
    };
    let energy = if target.energy.is_some() {
        context
            .hal
            .with_dchu(|io| energy::read(io, context.hal.reviewed_dsdt))
            .map(Some)
    } else {
        Ok(None)
    };
    let mut issue = match energy {
        Ok(current) => energy::restore_issue(target.energy, current),
        Err(error) => Some(format!("固件节能恢复前态不可读：{error:#}")),
    };
    if let Err(error) = target.bind_system(context.hal.system_power.as_ref()) {
        issue = Some(format!("系统计划恢复前态不可读：{error:#}"));
    }
    if let Some(initial) = target.gpu.clone() {
        let (node, clocks, driver, error) = gpu_preview(context, &initial);
        issue = issue.or(error);
        target.bind_gpu(node, clocks, driver)?;
    }
    Ok((Some(target), issue))
}
fn gpu_preview(
    context: &links::Context<'_>,
    initial: &links::Initial,
) -> (
    Option<crate::gpu_power::Device>,
    ClockObservation,
    Option<String>,
    Option<String>,
) {
    let mut current = None;
    let mut driver = None;
    let result: Result<ClockObservation> = (|| {
        let device = context.device()?;
        if let Some(device) = &device {
            node::validate_device(device)?;
        }
        current = device.clone();
        if let Some(initial) = &initial.node {
            let device = device.as_ref().context("原独显控制节点不可用")?;
            if initial.id != device.id {
                bail!("独显恢复基线属于其他设备");
            }
            crate::gpu_power::operation::preflight(device, device.target(), initial.target)?;
        }
        let Some(baseline) = &initial.clocks else {
            return Ok(ClockObservation::Untracked);
        };
        if device.as_ref().is_some_and(|device| !device.enabled) {
            return Ok(ClockObservation::PoweredOff);
        }
        let (reading, identity) = context
            .clocks
            .review_mode_gpu_restore_in_control(&baseline.state)?;
        driver = Some(identity);
        Ok(ClockObservation::Ready(reading))
    })();
    match result {
        Ok(clocks) => (current, clocks, driver, None),
        Err(error) => {
            let error = format!("GPU模式恢复前态不可用：{error:#}");
            (
                current,
                ClockObservation::Unavailable(error.clone()),
                None,
                Some(error),
            )
        }
    }
}
pub(crate) fn restore(
    context: links::Context<'_>,
    expected: &str,
    permit: &mut dyn FnMut() -> Result<()>,
) -> Result<()> {
    let session = context.session()?;
    let gate = session.context.hal.begin_write()?;
    permit()?;
    let (target, issue) = preview(&session.context)?;
    let target = target.context("没有初始模式快照")?;
    if target.revision != expected || issue.is_some() {
        bail!(
            "模式恢复预览已变化或不可恢复：{}",
            issue.as_deref().unwrap_or("请重新预览")
        );
    }
    let snapshot = session
        .context
        .hal
        .recovery()?
        .oem_mode
        .clone()
        .context("没有初始模式快照")?;
    let current_node = target
        .current_gpu_power
        .as_ref()
        .map(node::State::from_device)
        .transpose()?;
    if let Some(initial) = target.gpu.as_ref().and_then(|gpu| gpu.node.as_ref()) {
        session.freeze_restore(Some(initial), current_node.as_ref())?;
    }
    drop(gate);
    permit()?;
    session.authorize()?;
    let result = (|| {
        let _gate = session.context.hal.begin_write()?;
        permit()?;
        let (current, issue) = preview(&session.context)?;
        if issue.is_some()
            || current
                .as_ref()
                .is_none_or(|current| current.revision != expected)
        {
            bail!("授权后模式恢复目标已变化，未恢复任何关联域");
        }
        execute(&session, &snapshot, &target, permit)?;
        session.context.hal.recovery()?.oem_mode = None;
        Ok(())
    })();
    let result = match result {
        Err(error) => {
            let _gate = session.context.hal.begin_write()?;
            let recovery = session
                .rollback_nodes()
                .map_err(|error| format!("独显本次恢复失败：{error:#}"));
            Err(anyhow::anyhow!(ccc_core::msr::with_recovery(
                format!("{error:#}"),
                recovery
            )))
        }
        result => result,
    };
    match (result, session.finish()) {
        (Err(error), Err(cleanup)) => bail!("{error:#}；模式授权收尾失败：{cleanup:#}"),
        (Err(error), _) | (_, Err(error)) => Err(error),
        _ => Ok(()),
    }
}
fn execute(
    session: &links::Session<'_>,
    snapshot: &Snapshot,
    target: &workflow::RestoreTarget,
    permit: &mut dyn FnMut() -> Result<()>,
) -> Result<()> {
    let hal = session.context.hal;
    hal.ensure_mode_change_allowed()?;
    crate::fanctl::dust::ensure_manual_allowed(hal)?;
    energy::review_restore(hal, snapshot.energy)?;
    let mut core = snapshot.clone();
    let initial = core.links.take();
    let Some(initial) = initial else {
        return workflow::restore_snapshot(hal, &core);
    };
    let current_node = target
        .current_gpu_power
        .as_ref()
        .map(node::State::from_device)
        .transpose()?;
    let enabling = initial
        .node
        .as_ref()
        .is_some_and(|initial| initial.target.enabled)
        && target
            .current_gpu_power
            .as_ref()
            .is_some_and(|current| !current.enabled);
    if enabling {
        session.restore_node(initial.node.as_ref(), current_node.as_ref(), permit)?;
    }
    if let Some(baseline) = &initial.clocks {
        let actual = session
            .context
            .clocks
            .review_confirmed_mode_gpu_restore_in_control(&baseline.state, permit)?;
        if !enabling
            && !matches!(target.current_gpu_clocks, ClockObservation::PoweredOff)
            && target.current_gpu_clocks.raw() != Some(&actual)
        {
            bail!("GPU恢复前态在确认后已变化");
        }
        session
            .context
            .clocks
            .restore_mode_gpu_in_control(&baseline.state, &actual, permit)?;
    }
    permit()?;
    workflow::restore_snapshot(hal, &core)?;
    if !enabling && initial.node.is_some() {
        session.restore_node(initial.node.as_ref(), current_node.as_ref(), permit)?;
    }
    Ok(())
}
