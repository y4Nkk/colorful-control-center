use super::super::{
    model::{Snapshot, raw},
    transaction,
};
use crate::hal::nvapi::{
    clock::{LockEntry, LockTable},
    transaction::RawOffsets,
    turbo::{DeviceId, TurboIo},
};
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct RawState {
    pub devices: Vec<DeviceId>,
    pub locks: [LockEntry; 2],
    pub offsets: Vec<RawOffsets>,
}

impl RawState {
    pub(crate) fn projected(&self, target: &ccc_core::gpu_turbo::Target) -> Result<Self> {
        self.validate()?;
        let mut after = self.clone();
        if let Some(mhz) = target.lock_mhz {
            after.locks = LockTable::request_owned(self.locks)?
                .original_target(mhz)?
                .owned()?;
        }
        if let Some(offsets) = &target.offsets {
            if offsets.len() != after.offsets.len() {
                bail!("GPU偏移目标数量不一致");
            }
            for (current, wanted) in after.offsets.iter_mut().zip(offsets) {
                if current.core_khz.is_none() || current.mem_khz.is_none() {
                    bail!("GPU模式目标涉及未报告的偏移域");
                }
                current.core_khz = Some(
                    wanted
                        .core_mhz
                        .checked_mul(1000)
                        .ok_or_else(|| anyhow::anyhow!("GPU 核心频率偏移超出编码范围"))?,
                );
                current.mem_khz = Some(
                    wanted
                        .memory_mhz
                        .checked_mul(1000)
                        .ok_or_else(|| anyhow::anyhow!("GPU 显存频率偏移超出编码范围"))?,
                );
            }
        }
        Ok(after)
    }
    pub(crate) fn validate(&self) -> Result<()> {
        if self.devices.is_empty()
            || self.devices.len() > 64
            || self.offsets.len() != self.devices.len().min(2)
            || self
                .devices
                .iter()
                .enumerate()
                .any(|(i, id)| self.devices[..i].contains(id))
        {
            bail!("GPU频率快照的设备集合或偏移数量无效");
        }
        LockTable::request_owned(self.locks)?;
        Ok(())
    }
    pub(in crate::gpuctl::turbo) fn from_snapshot(snapshot: &Snapshot) -> Result<Self> {
        let state = Self {
            devices: snapshot.devices.clone(),
            locks: snapshot.locks.owned()?,
            offsets: snapshot.offsets.iter().map(raw).collect(),
        };
        state.validate()?;
        Ok(state)
    }
    pub(crate) fn read(io: &dyn TurboIo) -> Result<Self> {
        let devices = io.devices()?;
        if devices.is_empty() || io.primary()? != devices[0] {
            bail!("GPU频率控制会话与当前设备不一致");
        }
        let locks = io.locks(&devices[0])?.owned()?;
        let offsets = devices
            .iter()
            .take(2)
            .map(|id| io.offsets(id).map(|p| raw(&p)))
            .collect::<Result<Vec<_>>>()?;
        let state = Self {
            devices,
            locks,
            offsets,
        };
        state.validate()?;
        Ok(state)
    }
    pub(crate) fn matches_owned(&self, current: &Self) -> bool {
        self.devices == current.devices
            && self.locks == current.locks
            && self.offsets.len() == current.offsets.len()
            && self
                .offsets
                .iter()
                .zip(&current.offsets)
                .all(|(wanted, actual)| {
                    wanted.core_khz.is_none_or(|v| Some(v) == actual.core_khz)
                        && wanted.mem_khz.is_none_or(|v| Some(v) == actual.mem_khz)
                })
    }
    pub(crate) fn restore(
        &self,
        io: &dyn TurboIo,
        expected: &Self,
        permit: &mut dyn FnMut() -> Result<()>,
    ) -> Result<()> {
        self.review_restore(io, expected)?;
        let mut working = expected.clone();
        for index in (0..self.offsets.len()).rev() {
            self.restore_offsets(io, index, &mut working, permit)?;
        }
        if self.locks != working.locks {
            permit()?;
            working.recheck(io)?;
            let request =
                transaction::prepare_locks(io, &self.devices[0], self.locks, Some(working.locks))?;
            permit()?;
            transaction::send_locks(io, &self.devices[0], &request, self.locks)?;
        }
        if !self.matches_owned(&Self::read(io)?) {
            bail!("GPU频率恢复回读不一致，保留恢复记录");
        }
        Ok(())
    }
    pub(crate) fn review_restore(&self, io: &dyn TurboIo, expected: &Self) -> Result<()> {
        self.validate()?;
        expected.validate()?;
        if self.devices != expected.devices || Self::read(io)? != *expected {
            bail!("GPU频率恢复预览或设备已变化");
        }
        for (index, wanted) in self.offsets.iter().enumerate() {
            let current = io.offsets(&self.devices[index])?;
            for (value, actual) in [
                (wanted.core_khz, current.core),
                (wanted.mem_khz, current.memory),
            ] {
                let Some(value) =
                    value.filter(|value| *value != actual.delta_khz || !actual.present)
                else {
                    continue;
                };
                if !current.editable
                    || !current.p0_present
                    || current.count_clamped
                    || !actual.present
                    || !actual.editable
                    || value < actual.min_khz
                    || value > actual.max_khz
                {
                    bail!("GPU {} 原始偏移无法由当前驱动恢复", index + 1);
                }
            }
        }
        Ok(())
    }
    fn recheck(&self, io: &dyn TurboIo) -> Result<()> {
        if Self::read(io)? != *self {
            bail!("GPU频率在恢复过程中被更改");
        }
        Ok(())
    }
    fn restore_offsets(
        &self,
        io: &dyn TurboIo,
        index: usize,
        working: &mut Self,
        permit: &mut dyn FnMut() -> Result<()>,
    ) -> Result<()> {
        for (core, wanted) in [
            (false, self.offsets[index].mem_khz),
            (true, self.offsets[index].core_khz),
        ] {
            let Some(value) = wanted else { continue };
            let current = if core {
                working.offsets[index].core_khz
            } else {
                working.offsets[index].mem_khz
            };
            if current == Some(value) {
                continue;
            }
            if current.is_none() {
                bail!("GPU已接管的偏移域不可读");
            }
            permit()?;
            working.recheck(io)?;
            transaction::write_offset(io, &self.devices[index], core, value)?;
            if core {
                working.offsets[index].core_khz = Some(value);
            } else {
                working.offsets[index].mem_khz = Some(value);
            }
        }
        Ok(())
    }
}
