use super::super::{
    model::{self, Snapshot},
    source::{Source, SourceIo},
    transaction,
};
use super::{Policy, RawState};
use crate::{gpuctl::GpuCheckpoint, hal::nvapi::turbo::TurboIo};
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Write {
    pub before: RawState,
    pub after: RawState,
}

impl Write {
    pub(crate) fn validate_for_mode(
        &self,
        policy: &Policy,
        mode: u8,
        on_ac: bool,
        resume: bool,
    ) -> Result<()> {
        let decision = policy
            .decide(mode, Some(on_ac), resume)
            .map_err(anyhow::Error::msg)?;
        let target = decision
            .target
            .ok_or_else(|| anyhow::anyhow!("当前模式不允许GPU频率写入"))?;
        bind_devices(policy, &self.before.devices)?;
        self.validate_target(&target)
    }
    pub(crate) fn validate_target(&self, target: &ccc_core::gpu_turbo::Target) -> Result<()> {
        self.validate()?;
        if self.before.projected(target)? != self.after {
            bail!("GPU频率写集与已确认的原厂模式目标不一致");
        }
        Ok(())
    }
    pub(crate) fn validate(&self) -> Result<()> {
        self.before.validate()?;
        self.after.validate()?;
        if self.before.devices != self.after.devices
            || self
                .before
                .offsets
                .iter()
                .zip(&self.after.offsets)
                .any(|(a, b)| {
                    a.core_khz.is_some() != b.core_khz.is_some()
                        || a.mem_khz.is_some() != b.mem_khz.is_some()
                })
        {
            bail!("GPU频率写集改变了设备或未报告的偏移域");
        }
        Ok(())
    }
    pub(crate) fn may_recover(&self, actual: &RawState) -> bool {
        self.before.devices == actual.devices
            && self.before.offsets.len() == actual.offsets.len()
            && (0..2).all(|i| {
                actual.locks[i] == self.before.locks[i] || actual.locks[i] == self.after.locks[i]
            })
            && self
                .before
                .offsets
                .iter()
                .zip(&self.after.offsets)
                .zip(&actual.offsets)
                .all(|((a, b), c)| {
                    (c.core_khz == a.core_khz || c.core_khz == b.core_khz)
                        && (c.mem_khz == a.mem_khz || c.mem_khz == b.mem_khz)
                })
    }
}

pub(crate) struct Prepared {
    policy: Policy,
    identity: String,
    mode: u8,
    on_ac: bool,
    pub write: Write,
}

impl Prepared {
    pub(crate) fn driver_identity(&self) -> &str {
        &self.identity
    }
    pub(in crate::gpuctl::turbo) fn prepare(
        io: &dyn TurboIo,
        source: &dyn SourceIo,
        policy: &Policy,
        mode: u8,
        on_ac: bool,
        resume: bool,
    ) -> Result<Option<Self>> {
        if policy
            .decide(mode, Some(on_ac), resume)
            .map_err(anyhow::Error::msg)?
            .target
            .is_none()
        {
            return Ok(None);
        }
        let before = model::capture_with_base(io, source.read()?, false)?;
        check_source(&before.source, policy, on_ac)?;
        bind_devices(policy, &before.devices)?;
        let raw = RawState::from_snapshot(&before)?;
        let mut target = before.clone();
        target.source.mode = mode;
        let write = Write {
            before: raw,
            after: transaction::planned_state(&target, 97)?,
        };
        write.validate()?;
        Ok(Some(Self {
            policy: policy.clone(),
            identity: before.identity()?,
            mode,
            on_ac,
            write,
        }))
    }
    pub(in crate::gpuctl::turbo) fn execute(
        &self,
        io: &dyn TurboIo,
        source: &dyn SourceIo,
        state: &mut transaction::State,
        shared: &mut Option<GpuCheckpoint>,
        policy: &Policy,
        persist: &mut dyn FnMut(&Write) -> Result<()>,
        permit: &mut dyn FnMut() -> Result<()>,
    ) -> Result<()> {
        if policy != &self.policy {
            bail!("原厂GPU模式策略已变化");
        }
        permit()?;
        let current = self.recheck(io, source)?;
        if self.write.before != self.write.after {
            persist(&self.write)?;
        }
        permit()?;
        state.execute_captured(io, source, shared, &current, 97, permit)?;
        if RawState::read(io)? != self.write.after {
            bail!("原厂GPU模式调校的最终回读与预览目标不一致");
        }
        Ok(())
    }
    fn recheck(&self, io: &dyn TurboIo, source: &dyn SourceIo) -> Result<Snapshot> {
        let current = model::capture_with_base(io, source.read()?, false)?;
        check_source(&current.source, &self.policy, self.on_ac)?;
        bind_devices(&self.policy, &current.devices)?;
        if current.source.mode != self.mode
            || current.identity()? != self.identity
            || RawState::from_snapshot(&current)? != self.write.before
        {
            bail!("GPU设备、驱动能力、模式或频率前态已变化");
        }
        Ok(current)
    }
}

pub(super) fn check_source(source: &Source, policy: &Policy, on_ac: bool) -> Result<()> {
    if Some(source.capabilities) != policy.capabilities
        || source.limits != policy.limits
        || source.saved != policy.saved
        || source.on_ac != Some(on_ac)
    {
        bail!("原厂GPU能力、保存目标或供电已变化");
    }
    Ok(())
}
pub(super) fn bind_devices(
    policy: &Policy,
    devices: &[crate::hal::nvapi::turbo::DeviceId],
) -> Result<()> {
    if devices.is_empty()
        || devices.len() != usize::from(policy.gpu_count)
        || policy.cached_device_id != 0 && devices[0].extended as u16 != policy.cached_device_id
    {
        bail!("GPU实际集合或扩展设备编号与原厂报告不一致");
    }
    Ok(())
}
