use super::{
    Target,
    plans::{self, PlanValue},
};
use crate::gpu_power::mode as gpu;
use crate::powerctl::mode::workflow::StoredModeSnapshot;
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

const RECORD_VERSION: u8 = 5;
#[path = "record/clocks.rs"]
mod clocks;
#[path = "record/file.rs"]
mod file;
#[path = "record/initial.rs"]
mod initial;
use crate::gpuctl::turbo::mode::{
    ClockObservation, ClockOrigin, Policy as ClockPolicy, Write as ClockWrite,
};
pub(crate) use clocks::ClockApproval;
pub(super) use clocks::ClockRead;
pub(super) use file::{FileStore, Inspection};
pub(super) use initial::Initial;

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Snapshot {
    pub mode: StoredModeSnapshot,
    pub plans: Vec<PlanValue>,
    pub gpu_power: Option<gpu::State>,
    pub gpu_clocks: ClockObservation,
}
impl Snapshot {
    pub(super) fn validate(&self) -> Result<()> {
        if matches!(self.gpu_clocks, ClockObservation::Unavailable(_)) {
            bail!("读取失败的GPU观察不能保存为执行或恢复前态");
        }
        if self.mode.restore()?.core.ac_mode.is_some() {
            bail!("供电自动模式不能接管插电模式偏好");
        }
        plans::validate(&self.plans)?;
        if let Some(gpu) = &self.gpu_power {
            gpu.validate()?;
        }
        if let Some(clocks) = self.gpu_clocks.raw() {
            clocks.validate()?;
            if self
                .gpu_power
                .as_ref()
                .is_some_and(|gpu| !gpu.target.enabled)
            {
                bail!("关断状态不能冒充当前GPU频率实读");
            }
        }
        if matches!(self.gpu_clocks, ClockObservation::PoweredOff)
            && !self
                .gpu_power
                .as_ref()
                .is_some_and(|gpu| !gpu.target.enabled)
        {
            bail!("GPU频率观察未确认独显关闭");
        }
        self.mode.energy()?.validate()
    }
    pub(super) fn agrees_with(&self, current: &Self) -> bool {
        let mut expected = self.clone();
        if matches!(self.gpu_clocks, ClockObservation::Skipped(_))
            || matches!(current.gpu_clocks, ClockObservation::Skipped(_))
            || matches!(current.gpu_clocks, ClockObservation::Unavailable(_))
        {
            expected.gpu_clocks = current.gpu_clocks.clone();
        }
        expected == *current
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Attempt {
    pub before: Snapshot,
    pub target: Target,
    pub previous_mode: u8,
    pub on_ac: bool,
    pub trigger: ccc_core::oem_supply_mode::Trigger,
    pub gpu_action: Option<gpu::Action>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub clock_policy: Option<ClockPolicy>,
    pub clock_write: Option<ClockWrite>,
    pub clock_origin: Option<ClockOrigin>,
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Record {
    pub version: u8,
    pub enabled: bool,
    pub device_identity: String,
    pub identity: String,
    pub clock_approval: ClockApproval,
    pub capture_target: Option<Target>,
    pub initial: Option<Initial>,
    pub last: Option<Snapshot>,
    pub attempt: Option<Attempt>,
    pub track_clocks: bool,
}
impl Record {
    pub fn new(
        device_identity: String,
        identity: String,
        clock_approval: ClockApproval,
    ) -> Result<Self> {
        let record = Self {
            version: RECORD_VERSION,
            enabled: true,
            device_identity,
            identity,
            clock_approval,
            capture_target: None,
            initial: None,
            last: None,
            attempt: None,
            track_clocks: false,
        };
        record.validate()?;
        Ok(record)
    }
    fn validate(&self) -> Result<()> {
        if self.version != RECORD_VERSION {
            bail!(
                "供电模式恢复记录版本为 {}，当前版本为 {RECORD_VERSION}",
                self.version
            );
        }
        self.clock_approval.validate()?;
        if [&self.identity, &self.device_identity].iter().any(|value| {
            value.len() != 64
                || !value
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        }) || self.initial.is_none() != self.capture_target.is_none()
            || self.initial.is_none() && (self.last.is_some() || self.attempt.is_some())
        {
            bail!("供电模式恢复记录无效");
        }
        if let Some(last) = &self.last {
            last.validate()?;
        }
        if let Some(initial) = &self.initial {
            initial.validate()?;
            plans::validate_initial(&initial.mode, &initial.plans)?;
            if let Some(last) = &self.last {
                plans::changes(&initial.plans, &last.plans)?;
            }
        }
        if let Some(target) = &self.capture_target {
            validate_target(target)?;
        }
        if let Some(attempt) = &self.attempt {
            attempt.before.validate()?;
            let initial = self.initial.as_ref().context("供电模式初始快照缺失")?;
            plans::changes(&initial.plans, &attempt.before.plans)?;
            validate_target(&attempt.target)?;
            gpu::validate_action(
                attempt.gpu_action,
                attempt.before.gpu_power.as_ref(),
                attempt.target.mode,
                Some(attempt.trigger),
            )?;
            if Some(super::super::energy::desired(
                attempt.target.mode,
                attempt.before.mode.energy()?,
            )?) != attempt.target.energy
            {
                bail!("供电模式 EnergyStar 目标无效");
            }
            if !ccc_core::oem_events::is_power_mode(attempt.previous_mode) {
                bail!("供电模式前态无效");
            }
            validate_clock_attempt(attempt)?;
            if attempt.clock_decision()?.target.is_some() {
                self.clock_approval
                    .check_policy(attempt.clock_policy.as_ref().context("调频策略缺失")?)?;
            }
        }
        if !self.track_clocks
            && (self
                .initial
                .as_ref()
                .is_some_and(|i| i.gpu_clocks.is_some())
                || self
                    .last
                    .as_ref()
                    .is_some_and(|s| s.gpu_clocks.raw().is_some()))
        {
            bail!("未接管频率却存在恢复记录");
        }
        Ok(())
    }
}
fn validate_clock_attempt(attempt: &Attempt) -> Result<()> {
    let decision = attempt.clock_decision()?;
    let observation_matches = match (&attempt.before.gpu_clocks, decision.skipped) {
        (ClockObservation::Skipped(actual), Some(expected)) => *actual == expected,
        (ClockObservation::Ready(_) | ClockObservation::PoweredOff, None) => {
            decision.target.is_some()
        }
        _ => false,
    };
    if !observation_matches || attempt.clock_write.is_some() != attempt.clock_origin.is_some() {
        bail!("供电模式频率检查点无效");
    }
    if let Some(write) = &attempt.clock_write {
        write.validate_for_mode(
            attempt.clock_policy.as_ref().context("调频策略缺失")?,
            attempt.target.mode,
            attempt.on_ac,
            attempt.trigger == ccc_core::oem_supply_mode::Trigger::Resume,
        )?;
        validate_clock_origin(
            attempt.clock_origin.context("GPU检查点来源缺失")?,
            attempt,
            write,
        )?;
        if attempt
            .before
            .gpu_clocks
            .raw()
            .is_some_and(|before| before != &write.before)
        {
            bail!("频率写入检查点与本次原始快照不一致");
        }
    }
    Ok(())
}
impl Attempt {
    pub(super) fn clock_decision(&self) -> Result<crate::gpuctl::turbo::mode::Decision> {
        if self.trigger == ccc_core::oem_supply_mode::Trigger::Resume {
            if self.clock_policy.is_some() {
                bail!("唤醒不能携带未使用的GPU调频策略");
            }
            return Ok(crate::gpuctl::turbo::mode::Decision {
                target: None,
                skipped: Some(ccc_core::gpu_turbo::mode::SkipReason::Resume),
            });
        }
        self.clock_policy
            .as_ref()
            .context("GPU调频策略缺失")?
            .decide(self.target.mode, Some(self.on_ac), false)
            .map_err(anyhow::Error::msg)
    }
}
pub(super) fn validate_clock_origin(
    origin: ClockOrigin,
    attempt: &Attempt,
    write: &ClockWrite,
) -> Result<()> {
    let off = attempt
        .before
        .gpu_power
        .as_ref()
        .is_some_and(|state| !state.target.enabled);
    let enabling = attempt
        .gpu_action
        .is_some_and(|action| action.target.enabled && action.stage == gpu::Stage::BeforeMode);
    let valid = match origin {
        ClockOrigin::BeforeMode => attempt.before.gpu_clocks.raw() == Some(&write.before),
        ClockOrigin::AfterEnable => off && enabling && attempt.before.gpu_clocks.raw().is_none(),
        ClockOrigin::PoweredOff => off && !enabling && attempt.before.gpu_clocks.raw().is_none(),
    };
    if !valid || attempt.clock_decision()?.target.is_none() {
        bail!("GPU频率前态来源与供电事务不一致");
    }
    Ok(())
}
fn validate_target(target: &Target) -> Result<()> {
    let energy = target.energy.context("供电模式目标缺少EnergyStar所有权")?;
    energy.validate()?;
    if super::super::energy::desired(target.mode, energy)? != energy {
        bail!("供电模式节能目标无效");
    }
    if target.ac_mode.is_some()
        || target.from_mode.is_some()
        || target.gpu.is_some()
        || target.revision.is_some()
        || target.unavailable_reason.is_some()
        || target
            .aspm
            .is_some_and(|values| values.iter().any(|value| *value > 2))
        || target.power_plan.is_some_and(|value| value > 2)
    {
        bail!("供电模式目标无效");
    }
    target.fan.validate()?;
    target.fan.validate_turbo(target.turbo)
}
pub(super) fn update(
    store: &FileStore,
    record: &mut Record,
    change: impl FnOnce(&mut Record),
) -> Result<()> {
    let mut next = record.clone();
    change(&mut next);
    store.save(Some(record), &next)?;
    *record = next;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strict_store_uses_compare_and_swap_and_preserves_bad_files() {
        let dir = tempfile::tempdir().unwrap();
        let store = FileStore(dir.path().join("mode.json"));
        let mut record = Record::new(
            "b".repeat(64),
            "a".repeat(64),
            ClockApproval {
                policy_identity: "c".repeat(64),
                driver_identity: None,
            },
        )
        .unwrap();
        store.save(None, &record).unwrap();
        let original = record.clone();
        update(&store, &mut record, |record| record.enabled = false).unwrap();
        assert!(store.save(Some(&original), &original).is_err());
        let mut value = serde_json::to_value(&record).unwrap();
        value["unknown"] = true.into();
        std::fs::write(&store.0, serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(store.load().is_err());
        assert!(store.0.exists());
    }
}
