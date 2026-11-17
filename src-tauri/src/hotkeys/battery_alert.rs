mod clock;
mod codec;
mod codec_store;
#[cfg(test)]
mod codec_tests;
mod engine;
mod native;
#[cfg(test)]
mod tests;

use super::screen_brightness;
use crate::hal::Hal;
use anyhow::{Result, anyhow, ensure};
use serde::Serialize;
use std::{path::PathBuf, sync::Mutex, time::Instant};

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Profile {
    pub identity: Option<String>,
    pub binding: Option<engine::Binding>,
    pub brightness: Option<screen_brightness::LowBatteryProfile>,
    pub codec: codec::Profile,
    pub playing: bool,
    pub error: Option<String>,
}
#[derive(Default)]
struct State {
    engine: engine::Engine,
    fault: Option<String>,
}
pub(super) struct Controller {
    store: codec_store::Store,
    state: Mutex<State>,
}
impl Controller {
    pub fn new(path: PathBuf) -> Self {
        Self {
            store: codec_store::Store(path),
            state: Mutex::new(State::default()),
        }
    }
    fn scope(&self) -> Result<std::sync::MutexGuard<'_, State>> {
        self.state
            .lock()
            .map_err(|_| anyhow!("电池音频会话状态异常"))
    }
    pub fn read(&self, hal: &Hal, screen: &screen_brightness::Controller) -> Result<Profile> {
        let state = self.scope()?;
        let (binding, brightness, codec) = native::Native {
            hal,
            screen,
            store: &self.store,
        }
        .inspect();
        let error = binding.as_ref().err().map(|error| format!("{error:#}"));
        Ok(Profile {
            identity: binding
                .as_ref()
                .ok()
                .and_then(|binding| binding.identity().ok()),
            binding: binding.ok(),
            brightness: brightness.ok(),
            codec,
            playing: state.engine.playing(),
            error: error.or_else(|| state.fault.clone()),
        })
    }
    pub fn start(&self) -> Result<()> {
        let mut state = self.scope()?;
        ensure!(!state.engine.playing(), "上次音频停止未完成，请先恢复");
        *state = State::default();
        Ok(())
    }
    pub fn event(
        &self,
        hal: &Hal,
        screen: &screen_brightness::Controller,
        expected: &str,
        active: bool,
        brightness: Option<u8>,
        mut permit: impl FnMut() -> Result<()>,
    ) -> Result<()> {
        let mut state = self.scope()?;
        let io = native::Native {
            hal,
            screen,
            store: &self.store,
        };
        let result = state.engine.event(
            &io,
            expected,
            active,
            brightness,
            &mut Instant::now,
            &mut permit,
        );
        self.finish(&mut state, &io, result)
    }
    pub fn tick(
        &self,
        hal: &Hal,
        screen: &screen_brightness::Controller,
        mut permit: impl FnMut() -> Result<()>,
    ) -> Result<bool> {
        let mut state = self.scope()?;
        let io = native::Native {
            hal,
            screen,
            store: &self.store,
        };
        let result = state.engine.tick(&io, Instant::now(), &mut permit);
        match result {
            Ok(changed) => Ok(changed),
            Err(error) => self.finish(&mut state, &io, Err(error)).map(|_| false),
        }
    }
    fn finish(&self, state: &mut State, io: &native::Native<'_>, result: Result<()>) -> Result<()> {
        if let Err(error) = result {
            let cleanup = cleanup(state, io);
            let message = ccc_core::msr::with_recovery(
                format!("{error:#}"),
                cleanup.map_err(|error| format!("{error:#}")),
            );
            state.fault = Some(message.clone());
            anyhow::bail!(message);
        }
        Ok(())
    }
    pub fn cleanup(&self, hal: &Hal, screen: &screen_brightness::Controller) -> Result<()> {
        let mut state = self.scope()?;
        let result = cleanup(
            &mut state,
            &native::Native {
                hal,
                screen,
                store: &self.store,
            },
        );
        state.fault = result.as_ref().err().map(|error| format!("{error:#}"));
        result
    }
    pub fn audio(
        &self,
        hal: &Hal,
        screen: &screen_brightness::Controller,
    ) -> Result<codec::Profile> {
        let _state = self.scope()?;
        Ok(native::Native {
            hal,
            screen,
            store: &self.store,
        }
        .codec(true))
    }
    pub fn restore(
        &self,
        hal: &Hal,
        screen: &screen_brightness::Controller,
        expected: &str,
    ) -> Result<()> {
        let mut state = self.scope()?;
        let io = native::Native {
            hal,
            screen,
            store: &self.store,
        };
        ensure!(
            io.codec(true).restore_revision.as_deref() == Some(expected),
            "电池音频恢复预览已变化"
        );
        state.engine.cancel(&io)?;
        io.restore_codec(expected)?;
        state.fault = None;
        Ok(())
    }
}
fn cleanup(state: &mut State, io: &native::Native<'_>) -> Result<()> {
    let stopped = state.engine.cancel(io);
    let codec = io.cleanup_codec();
    match (stopped, codec) {
        (Err(first), Err(second)) => {
            anyhow::bail!("停止静音循环：{first:#}；恢复PC Beep：{second:#}")
        }
        (Err(error), _) | (_, Err(error)) => Err(error),
        _ => Ok(()),
    }
}
