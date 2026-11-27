use super::{
    Review, Scope, Target,
    plans::PlanValue,
    record::{self, Attempt, ClockApproval, ClockRead, FileStore, Initial, Record, Snapshot},
};
use crate::gpu_power::mode as gpu;
use crate::gpuctl::turbo::mode::{ClockObservation, ClockOrigin, RawState, Write as ClockWrite};
use crate::powerctl::mode::workflow::StoredModeSnapshot;
use anyhow::{Result, anyhow, bail};
use ccc_core::oem_supply_mode::Trigger;
#[path = "transaction/clocks.rs"]
mod clocks;
mod restore;
mod view;
pub(super) use restore::restore;
pub(super) use view::{View, read, read_scoped};

pub(super) trait Io {
    fn review(&self, scope: Scope) -> Result<Option<Review>>;
    fn device_identity(&self) -> Result<String>;
    fn snapshot(&self, target: &Target) -> Result<StoredModeSnapshot>;
    fn initial_plans(
        &self,
        review: &Review,
        initial: &StoredModeSnapshot,
    ) -> Result<Vec<PlanValue>>;
    fn plans(&self, initial: &[PlanValue]) -> Result<Vec<PlanValue>>;
    fn gpu_power(&self) -> Result<Option<gpu::State>>;
    fn clocks(&self) -> Result<Option<RawState>>;
    fn review_gpu_restore(
        &self,
        initial: Option<&gpu::State>,
        current: Option<&gpu::State>,
    ) -> Result<()>;
    fn apply(
        &self,
        attempt: &Attempt,
        capture: &Target,
        checkpoint: &mut dyn FnMut(&ClockWrite, ClockOrigin, &str) -> Result<()>,
        permit: &mut dyn FnMut() -> Result<()>,
    ) -> Result<()>;
    fn restore(
        &self,
        initial: &Initial,
        capture: &Target,
        expected: &Snapshot,
        permit: &mut dyn FnMut() -> Result<()>,
    ) -> Result<()>;
}

pub(super) fn capture(
    io: &impl Io,
    target: &Target,
    plans: &[PlanValue],
    clocks: ClockRead,
) -> Result<Snapshot> {
    Ok(Snapshot {
        mode: io.snapshot(target)?,
        plans: io.plans(plans)?,
        gpu_power: io.gpu_power()?,
        gpu_clocks: clocks.capture(io)?,
    })
}

pub(super) fn enable(
    io: &impl Io,
    store: &FileStore,
    expected: &str,
    permit: &mut dyn FnMut() -> Result<()>,
) -> Result<()> {
    let view = read(io, store)?;
    if view.revision != expected || !view.can_enable {
        bail!("供电模式预览已变化或不可启用");
    }
    let review = view.review.ok_or_else(|| anyhow!("供电模式不可用"))?;
    permit()?;
    let approval = ClockApproval::new(
        review
            .clocks
            .as_ref()
            .ok_or_else(|| anyhow!("GPU调频预览缺失"))?,
    )?;
    store.save(
        None,
        &Record::new(review.device_identity, review.identity, approval)?,
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Applied {
    pub previous_mode: u8,
    pub mode: u8,
    pub on_ac: bool,
    pub trigger: Trigger,
}

pub(super) fn execute(
    io: &impl Io,
    store: &FileStore,
    trigger: Trigger,
    permit: &mut dyn FnMut() -> Result<()>,
) -> Result<Option<Applied>> {
    let mut record = store.load()?.ok_or_else(|| anyhow!("供电模式尚未启用"))?;
    let scope = Scope::Event(trigger);
    let review = io
        .review(scope)?
        .ok_or_else(|| anyhow!("原厂供电模式不可用"))?;
    if !record.enabled
        || record.attempt.is_some()
        || record.identity != review.identity
        || record.device_identity != review.device_identity
    {
        bail!("供电模式目标或接管状态已变化");
    }
    review.check_clocks(scope, &record.clock_approval)?;
    if let Some(reason) = review.unavailable_for(scope) {
        bail!("原厂模式联动未通过预检：{reason}");
    }
    let on_ac = review.on_ac.ok_or_else(|| anyhow!("供电状态未知"))?;
    let target = review.target(trigger, on_ac).cloned();
    let Some(target) = target else {
        return Ok(None);
    };
    let (capture, attempt) = begin(io, store, &mut record, &review, &target, trigger, permit)?;
    if let Err(error) = io.apply(
        &attempt,
        &capture,
        &mut |write, origin, driver| clocks::checkpoint(store, &mut record, write, origin, driver),
        permit,
    ) {
        if let Err(record_error) =
            clear_if_unchanged(io, store, &mut record, &capture, &attempt.before)
        {
            bail!("{error:#}；供电事务记录核对失败：{record_error:#}");
        }
        return Err(error);
    }
    finish(io, store, &mut record, &review, &target, &capture)?;
    Ok(Some(Applied {
        previous_mode: review.current_mode,
        mode: target.mode,
        on_ac,
        trigger,
    }))
}

fn begin(
    io: &impl Io,
    store: &FileStore,
    record: &mut Record,
    review: &Review,
    target: &Target,
    trigger: Trigger,
    permit: &mut dyn FnMut() -> Result<()>,
) -> Result<(Target, Attempt)> {
    let capture = record
        .capture_target
        .clone()
        .unwrap_or_else(|| target.clone());
    let mode = io.snapshot(&capture)?;
    let (clock_policy, clock_read) = clock_capture(review, target, trigger)?;
    let plans = match &record.initial {
        Some(initial) => io.plans(&initial.plans)?,
        None => io.initial_plans(review, &mode)?,
    };
    let before = Snapshot {
        mode,
        plans,
        gpu_power: io.gpu_power()?,
        gpu_clocks: clock_read.capture(io)?,
    };
    check_before(record, review, &before)?;
    let candidate = review.gpu.candidate(
        trigger,
        review.on_ac.ok_or_else(|| anyhow!("供电状态未知"))?,
    );
    if let Some(reason) = &candidate.unavailable_reason {
        bail!("供电模式独显联动不可执行：{reason}");
    }
    let attempt = Attempt {
        before: before.clone(),
        target: target.clone(),
        previous_mode: review.current_mode,
        on_ac: review.on_ac.ok_or_else(|| anyhow!("供电状态未知"))?,
        trigger,
        gpu_action: candidate.action,
        clock_policy,
        clock_write: None,
        clock_origin: None,
    };
    permit()?;
    record::update(store, record, |record| {
        if record.initial.is_none() {
            record.initial = Some(Initial::new(before.clone()));
            record.capture_target = Some(capture.clone());
        }
        record.track_clocks |= matches!(clock_read, ClockRead::Tracked);
        record.attempt = Some(attempt.clone());
    })?;
    Ok((capture, attempt))
}

fn clock_capture(
    review: &Review,
    target: &Target,
    trigger: Trigger,
) -> Result<(Option<crate::gpuctl::turbo::mode::Policy>, ClockRead)> {
    if trigger == Trigger::Resume {
        return Ok((
            None,
            ClockRead::Skip(ccc_core::gpu_turbo::mode::SkipReason::Resume),
        ));
    }
    let policy = review
        .clocks
        .as_ref()
        .ok_or_else(|| anyhow!("GPU调频预览缺失"))?
        .policy
        .clone();
    let decision = policy
        .decide(target.mode, review.on_ac, false)
        .map_err(anyhow::Error::msg)?;
    let read = match decision.skipped {
        Some(reason) => ClockRead::Skip(reason),
        None => ClockRead::Tracked,
    };
    Ok((Some(policy), read))
}

fn check_before(record: &Record, review: &Review, before: &Snapshot) -> Result<()> {
    let mut comparable = before.clone();
    if !record.track_clocks {
        comparable.gpu_clocks = record
            .last
            .as_ref()
            .map(|last| last.gpu_clocks.clone())
            .unwrap_or(ClockObservation::Untracked);
    }
    if review.current_mode != before.mode.current_mode()
        || review.current_energy != before.mode.energy()?
        || record
            .last
            .as_ref()
            .is_some_and(|last| !last.agrees_with(&comparable))
    {
        bail!("模式、风扇、电源计划、EnergyStar、独显或频率状态已变化，请重新预览");
    }
    if let Some(actual) = before.gpu_clocks.raw() {
        if record
            .initial
            .as_ref()
            .and_then(|initial| initial.gpu_clocks.as_ref())
            .is_some_and(|baseline| baseline.state.devices != actual.devices)
        {
            bail!("GPU频率恢复基线属于其他设备");
        }
        if review
            .clocks
            .as_ref()
            .and_then(|clocks| clocks.driver.as_ref())
            .is_some_and(|driver| &driver.current != actual)
        {
            bail!("GPU频率在模式预览后已变化");
        }
    }
    if review
        .gpu
        .current
        .as_ref()
        .map(gpu::State::from_device)
        .transpose()?
        != before.gpu_power
    {
        bail!("独显前态已变化，请重新预览");
    }
    Ok(())
}

fn clear_if_unchanged(
    io: &impl Io,
    store: &FileStore,
    record: &mut Record,
    target: &Target,
    before: &Snapshot,
) -> Result<()> {
    if capture(
        io,
        target,
        &before.plans,
        ClockRead::for_observation(&before.gpu_clocks)?,
    )? != *before
    {
        bail!("模式恢复未完成，保留本次事务记录");
    }
    record::update(store, record, |record| {
        record.attempt = None;
        if record.last.is_none() {
            record.initial = None;
            record.capture_target = None;
            record.track_clocks = false;
        }
    })
}

fn finish(
    io: &impl Io,
    store: &FileStore,
    record: &mut Record,
    before: &Review,
    target: &Target,
    sampling: &Target,
) -> Result<()> {
    let initial = record
        .initial
        .as_ref()
        .ok_or_else(|| anyhow!("初始供电快照缺失"))?;
    let attempt = record
        .attempt
        .as_ref()
        .ok_or_else(|| anyhow!("供电事务检查点缺失"))?;
    let scope = Scope::Event(attempt.trigger);
    let after = capture(
        io,
        sampling,
        &initial.plans,
        ClockRead::for_observation(&attempt.before.gpu_clocks)?,
    )?;
    let next = io
        .review(scope)?
        .ok_or_else(|| anyhow!("模式已应用但供电预览不可用，保留事务记录"))?;
    let gpu_expected = record
        .attempt
        .as_ref()
        .and_then(|attempt| attempt.gpu_action)
        .map(|action| action.target)
        .or_else(|| {
            record
                .attempt
                .as_ref()
                .and_then(|attempt| attempt.before.gpu_power.as_ref().map(|state| state.target))
        });
    let accepted = equivalent_after_own_change(before, &next, target)
        && next.check_clocks(scope, &record.clock_approval).is_ok()
        && clocks::confirmed(
            record
                .attempt
                .as_ref()
                .ok_or_else(|| anyhow!("供电事务检查点缺失"))?,
            &after,
        )?
        && next.current_energy == after.mode.energy()?
        && after.gpu_power.as_ref().map(|state| state.target) == gpu_expected
        && next
            .gpu
            .current
            .as_ref()
            .map(gpu::State::from_device)
            .transpose()?
            == after.gpu_power;
    record::update(store, record, |record| {
        record.last = Some(after);
        record.attempt = None;
        if accepted {
            record.identity = next.identity;
        } else {
            record.enabled = false;
        }
    })?;
    if !accepted {
        bail!("模式已应用，但原厂目标或设备发生变化，自动切换已暂停");
    }
    Ok(())
}

fn equivalent_after_own_change(before: &Review, after: &Review, applied: &Target) -> bool {
    if before.device_identity != after.device_identity
        || before.ac_mode != after.ac_mode
        || after.current_mode != applied.mode
        || before.gpu.identity() != after.gpu.identity()
        || before.clocks.as_ref().map(|clocks| &clocks.policy)
            != after.clocks.as_ref().map(|clocks| &clocks.policy)
        || before.current_energy.hardware_p_state != after.current_energy.hardware_p_state
        || applied.energy != Some(after.current_energy)
        || before.targets.len() != after.targets.len()
    {
        return false;
    }
    before.targets.iter().zip(&after.targets).all(|(old, new)| {
        if old == new {
            return true;
        }
        use crate::fanctl::{FAN_MODE_AUTO, FAN_MODE_FULL_SPEED, FAN_MODE_NOISELESS, ModePolicy};
        let mut adjusted = old.clone();
        adjusted.fan = new.fan;
        old.mode == applied.mode
            && old.fan
                == ModePolicy::explicit(
                    FAN_MODE_FULL_SPEED,
                    FAN_MODE_FULL_SPEED,
                    old.fan.offset_pct(),
                )
            && matches!(
                new.fan,
                ModePolicy::Explicit {
                    mode: FAN_MODE_AUTO | FAN_MODE_NOISELESS,
                    saved_mode: FAN_MODE_AUTO,
                    ..
                }
            )
            && new.fan.offset_pct() == old.fan.offset_pct()
            && *new == adjusted
    })
}
