use super::{Policy, Prepared, RawState, Write, execution};
use crate::gpuctl::turbo::{ensure_turbo, source::SourceIo};
use crate::gpuctl::{ControllerState, GpuController, connect_session, disconnect_session};
use crate::hal::nvapi::turbo::TurboIo;
use anyhow::{Result, anyhow, bail};

// 调用者持有HAL控制锁；这里仅取得GPU领域锁，避免重复取得HAL。
impl GpuController {
    pub(crate) fn prepare_mode_gpu_in_control(
        &self,
        source: &dyn SourceIo,
        policy: &Policy,
        mode: u8,
        on_ac: bool,
        resume: bool,
        permit: &mut dyn FnMut() -> Result<()>,
    ) -> Result<Option<Prepared>> {
        if policy
            .decide(mode, Some(on_ac), resume)
            .map_err(anyhow::Error::msg)?
            .target
            .is_none()
        {
            return Ok(None);
        }
        let mut state = self.state.lock().map_err(|_| anyhow!("GPU控制状态异常"))?;
        confirmed_access(&mut state, permit, |state, io| {
            let prepared = execution::Prepared::prepare(io, source, policy, mode, on_ac, resume)?;
            if let Some(prepared) = &prepared {
                let devices = &prepared.write.before.devices;
                if state
                    .turbo
                    .initial
                    .as_ref()
                    .is_some_and(|initial| &initial.devices != devices)
                    || state
                        .baseline
                        .is_some_and(|baseline| baseline.device != devices[0])
                {
                    bail!("GPU初始恢复记录属于其他设备");
                }
            }
            Ok(prepared)
        })
    }
    pub(crate) fn apply_mode_gpu_in_control(
        &self,
        source: &dyn SourceIo,
        policy: &Policy,
        prepared: &Prepared,
        persist: &mut dyn FnMut(&Write) -> Result<()>,
        permit: &mut dyn FnMut() -> Result<()>,
    ) -> Result<()> {
        let mut state = self.state.lock().map_err(|_| anyhow!("GPU控制状态异常"))?;
        permit()?;
        confirmed_access(&mut state, &mut || Ok(()), |state, io| {
            let ControllerState {
                turbo, baseline, ..
            } = state;
            prepared.execute(io, source, turbo, baseline, policy, persist, permit)
        })
    }
    pub(crate) fn read_mode_gpu_in_control(&self) -> Result<RawState> {
        let mut state = self.state.lock().map_err(|_| anyhow!("GPU控制状态异常"))?;
        RawState::read(ensure_turbo(&mut state)?.as_ref())
    }
    pub(crate) fn restore_confirmed_checkpoint_in_control(
        &self,
        before: &crate::gpuctl::GpuCheckpoint,
        permit: &mut dyn FnMut() -> Result<()>,
    ) -> Result<()> {
        let mut state = self.state.lock().map_err(|_| anyhow!("GPU控制状态异常"))?;
        confirmed_access(&mut state, permit, |state, io| {
            if io.primary()? != before.device {
                bail!("GPU身份已变化，未恢复其他设备的偏移快照");
            }
            let offsets = connect_session(state)?;
            let actual = offsets.read()?;
            if !actual.p0_present
                || before.core_khz.is_some() && !actual.core.present
                || before.mem_khz.is_some() && !actual.memory.present
            {
                bail!("GPU偏移恢复所需的P0时钟域已不可读");
            }
            let changed = crate::hal::nvapi::transaction::RawOffsets {
                core_khz: before
                    .core_khz
                    .filter(|value| *value != actual.core.delta_khz),
                mem_khz: before
                    .mem_khz
                    .filter(|value| *value != actual.memory.delta_khz),
            };
            if changed.core_khz.is_none() && changed.mem_khz.is_none() {
                return Ok(());
            }
            crate::hal::nvapi::transaction::restore_raw(offsets.as_ref(), &changed)
        })
    }
    pub(crate) fn read_confirmed_mode_gpu_in_control(
        &self,
        permit: &mut dyn FnMut() -> Result<()>,
    ) -> Result<RawState> {
        let mut state = self.state.lock().map_err(|_| anyhow!("GPU控制状态异常"))?;
        confirmed_access(&mut state, permit, |_, io| RawState::read(io))
    }
    pub(crate) fn review_mode_gpu_restore_in_control(
        &self,
        initial: &RawState,
    ) -> Result<(RawState, String)> {
        let mut state = self.state.lock().map_err(|_| anyhow!("GPU控制状态异常"))?;
        let io = ensure_turbo(&mut state)?;
        let current = RawState::read(io.as_ref())?;
        initial.review_restore(io.as_ref(), &current)?;
        let identity = current.restore_identity(io.as_ref())?;
        Ok((current, identity))
    }
    pub(crate) fn review_confirmed_mode_gpu_restore_in_control(
        &self,
        initial: &RawState,
        permit: &mut dyn FnMut() -> Result<()>,
    ) -> Result<RawState> {
        let mut state = self.state.lock().map_err(|_| anyhow!("GPU控制状态异常"))?;
        confirmed_access(&mut state, permit, |_, io| {
            let current = RawState::read(io)?;
            initial.review_restore(io, &current)?;
            Ok(current)
        })
    }
    pub(crate) fn restore_mode_gpu_in_control(
        &self,
        initial: &RawState,
        expected: &RawState,
        permit: &mut dyn FnMut() -> Result<()>,
    ) -> Result<()> {
        let mut state = self.state.lock().map_err(|_| anyhow!("GPU控制状态异常"))?;
        permit()?;
        confirmed_access(&mut state, &mut || Ok(()), |_, io| {
            initial.restore(io, expected, permit)
        })
    }
}

struct ControlAccess<'a>(&'a mut ControllerState);
impl Drop for ControlAccess<'_> {
    fn drop(&mut self) {
        if self.0.power_suspended {
            disconnect_session(self.0);
        }
    }
}
fn confirmed_access<T>(
    state: &mut ControllerState,
    permit: &mut dyn FnMut() -> Result<()>,
    action: impl FnOnce(&mut ControllerState, &dyn TurboIo) -> Result<T>,
) -> Result<T> {
    permit()?;
    let access = ControlAccess(state);
    connect_session(access.0)?;
    let io = access
        .0
        .turbo_io
        .clone()
        .ok_or_else(|| anyhow!("GPU模式控制接口不可用"))?;
    action(access.0, io.as_ref())
}
