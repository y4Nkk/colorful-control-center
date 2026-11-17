#[path = "turbo/mode.rs"]
pub(crate) mod mode;
#[path = "turbo/model.rs"]
mod model;
#[path = "turbo/source.rs"]
pub(crate) mod source;
#[path = "turbo/transaction.rs"]
mod transaction;

use super::{ControllerState, GpuController, ensure_session};
use crate::hal::{Hal, nvapi::turbo::TurboIo};
use anyhow::{Context, Result, anyhow};
use std::sync::Arc;

pub use model::Profile;
pub(super) use transaction::State;

fn ensure_turbo(state: &mut ControllerState) -> Result<Arc<dyn TurboIo>> {
    ensure_session(state)?;
    state
        .turbo_io
        .clone()
        .context("GPU Turbo 接口不可用；隔离环境必须显式注入")
}
pub(super) fn current_device(
    state: &mut ControllerState,
) -> Result<crate::hal::nvapi::turbo::DeviceId> {
    let io = ensure_turbo(state)?;
    let devices = io.devices()?;
    let primary = io.primary()?;
    if devices.first() != Some(&primary) {
        anyhow::bail!("GPU 控制会话与当前第一张显卡不一致");
    }
    Ok(primary)
}

impl GpuController {
    pub(crate) fn read_turbo(&self, hal: &Hal) -> Result<Profile> {
        let _gate = hal.begin_write()?;
        let mut state = self.state.lock().map_err(|_| anyhow!("GPU 控制状态异常"))?;
        let io = ensure_turbo(&mut state).and_then(|io| {
            let device = current_device(&mut state)?;
            if state
                .baseline
                .is_some_and(|baseline| baseline.device != device)
            {
                anyhow::bail!("GPU 初始偏移属于其他设备，请连接原显卡后恢复");
            }
            Ok(io)
        });
        match io {
            Ok(io) => Ok(state.turbo.preview(io.as_ref(), &source::Native(hal))),
            Err(error) => {
                let error = format!("{error:#}");
                Ok(Profile {
                    identity: None,
                    current: None,
                    error: Some(error.clone()),
                    initial: state.turbo.initial.clone(),
                    restore_current: None,
                    restore_revision: None,
                    restore_error: state.turbo.initial.as_ref().map(|_| error),
                })
            }
        }
    }

    pub(crate) fn run_turbo(
        &self,
        hal: &Hal,
        expected: &str,
        event: u8,
        permit: impl FnMut() -> Result<()>,
    ) -> Result<String> {
        let _gate = hal.begin_write()?;
        let mut state = self.state.lock().map_err(|_| anyhow!("GPU 控制状态异常"))?;
        let io = ensure_turbo(&mut state)?;
        let ControllerState {
            turbo, baseline, ..
        } = &mut *state;
        turbo.execute(
            io.as_ref(),
            &source::Native(hal),
            baseline,
            expected,
            event,
            permit,
        )
    }

    pub(crate) fn restore_turbo(&self, hal: &Hal, expected: &str) -> Result<()> {
        let _gate = hal.begin_write()?;
        let mut state = self.state.lock().map_err(|_| anyhow!("GPU 控制状态异常"))?;
        let io = ensure_turbo(&mut state)?;
        state.turbo.restore(io.as_ref(), expected)
    }
}

#[cfg(test)]
#[path = "turbo/tests.rs"]
mod tests;
