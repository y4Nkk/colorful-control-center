use crate::hal::key_events;
use anyhow::{Result, anyhow, bail};
use ccc_core::keymap::{
    Step,
    recording::{Outcome, Recorder},
};
use serde::Serialize;
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

const LIMIT: Duration = Duration::from_secs(120);
struct State {
    engine: Recorder,
    started: Instant,
    finished_at: Option<Duration>,
    reason: Option<String>,
}
impl State {
    fn finish(&mut self, reason: &str) -> Result<()> {
        if self.engine.done() {
            return Ok(());
        }
        let elapsed = self.started.elapsed();
        self.engine.finish(elapsed).map_err(|e| anyhow!(e))?;
        self.finished_at = Some(elapsed);
        self.reason = Some(reason.into());
        Ok(())
    }
}
struct Capture {
    state: Arc<Mutex<State>>,
    input: Option<key_events::Session>,
}
#[derive(Default)]
pub struct Controller {
    capture: Mutex<Option<Capture>>,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    active: bool,
    steps: Vec<Step>,
    reason: Option<String>,
    elapsed_ms: u64,
}
fn snapshot(state: &State) -> Snapshot {
    Snapshot {
        active: !state.engine.done(),
        steps: state.engine.steps().to_vec(),
        reason: state.reason.clone(),
        elapsed_ms: state
            .finished_at
            .unwrap_or_else(|| state.started.elapsed())
            .as_millis() as u64,
    }
}
fn event(
    state: &Arc<Mutex<State>>,
    stop: &Arc<Mutex<Option<key_events::Stopper>>>,
    key: key_events::KeyEvent,
) -> bool {
    if key.injected || matches!(key.scan, 0xe077 | 0xe078) {
        return false;
    }
    let Ok(mut state) = state.lock() else {
        return false;
    };
    if state.engine.done() {
        return false;
    }
    let result = if state.started.elapsed() >= LIMIT {
        state.finish("录制已达到2分钟上限")
    } else if key.scan == 1 && key.down {
        state.finish("已按Esc停止录制")
    } else {
        let elapsed = key.at.saturating_duration_since(state.started);
        match state.engine.capture(key.scan, key.down, elapsed) {
            Ok(Outcome::LimitReached) => {
                state.finished_at = Some(elapsed);
                state.reason = Some("录制已达到256步，已补齐释放动作".into());
                Ok(())
            }
            Ok(_) => Ok(()),
            Err(error) => state.finish(&format!("录制已停止：{error}")),
        }
    };
    if let Err(error) = result {
        state.reason = Some(format!("结束录制失败：{error:#}"));
    }
    if state.engine.done() {
        if let Ok(stop) = stop.lock() {
            if let Some(stop) = stop.as_ref() {
                stop.stop();
            }
        }
    }
    true
}
fn observe(
    state: &Arc<Mutex<State>>,
    stop: &Arc<Mutex<Option<key_events::Stopper>>>,
    key: key_events::KeyEvent,
) -> bool {
    event(state, stop, key);
    // Consuming a release for a key held before capture can leave modifiers stuck.
    false
}
impl Controller {
    pub fn start(&self) -> Result<Snapshot> {
        let mut slot = self.capture.lock().map_err(|_| anyhow!("录制状态异常"))?;
        if slot
            .as_ref()
            .is_some_and(|c| c.state.lock().map_or(true, |s| !s.engine.done()))
        {
            bail!("已有宏录制正在进行");
        }
        slot.take();
        let state = Arc::new(Mutex::new(State {
            engine: Recorder::default(),
            started: Instant::now(),
            finished_at: None,
            reason: None,
        }));
        let stop = Arc::new(Mutex::new(None));
        let events = state.clone();
        let stopping = stop.clone();
        let failed = state.clone();
        let input = key_events::Session::start(
            move |key| observe(&events, &stopping, key),
            move |error| {
                if let Ok(mut state) = failed.lock() {
                    let _ = state.finish(&error);
                }
            },
        )?;
        *stop.lock().map_err(|_| anyhow!("录制停止状态异常"))? = Some(input.stopper());
        let result = {
            let state = state.lock().map_err(|_| anyhow!("录制状态异常"))?;
            snapshot(&state)
        };
        if !result.active {
            input.stopper().stop();
        }
        *slot = Some(Capture {
            state,
            input: Some(input),
        });
        Ok(result)
    }
    pub fn read(&self) -> Result<Option<Snapshot>> {
        let mut slot = self.capture.lock().map_err(|_| anyhow!("录制状态异常"))?;
        let Some(capture) = slot.as_mut() else {
            return Ok(None);
        };
        let result = {
            let mut state = capture.state.lock().map_err(|_| anyhow!("录制状态异常"))?;
            if !state.engine.done() && state.started.elapsed() >= LIMIT {
                state.finish("录制已达到2分钟上限")?;
            }
            if let Some(error) = capture.input.as_ref().and_then(key_events::Session::error) {
                state.finish(&error)?;
            }
            snapshot(&state)
        };
        let input = if !result.active {
            capture.input.take()
        } else {
            None
        };
        drop(slot);
        drop(input);
        Ok(Some(result))
    }
    pub fn stop(&self) -> Result<Option<Snapshot>> {
        let mut slot = self.capture.lock().map_err(|_| anyhow!("录制状态异常"))?;
        let Some(capture) = slot.as_mut() else {
            return Ok(None);
        };
        let result = {
            let mut state = capture.state.lock().map_err(|_| anyhow!("录制状态异常"))?;
            state.finish("已停止录制")?;
            snapshot(&state)
        };
        let input = capture.input.take();
        drop(slot);
        drop(input);
        Ok(Some(result))
    }
    pub fn active(&self) -> Result<bool> {
        Ok(self.read()?.is_some_and(|s| s.active))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn state() -> Arc<Mutex<State>> {
        Arc::new(Mutex::new(State {
            engine: Recorder::default(),
            started: Instant::now(),
            finished_at: None,
            reason: None,
        }))
    }
    fn key(scan: u16, down: bool) -> key_events::KeyEvent {
        key_events::KeyEvent {
            scan,
            down,
            injected: false,
            at: Instant::now(),
        }
    }
    #[test]
    fn escape_stops_without_recording_escape_and_synthesizes_only_draft_releases() {
        let state = state();
        let stop = Arc::new(Mutex::new(None));
        assert!(event(&state, &stop, key(29, true)));
        assert!(event(&state, &stop, key(1, true)));
        let snapshot = snapshot(&state.lock().unwrap());
        assert!(!snapshot.active);
        assert_eq!(snapshot.steps.len(), 2);
        assert!(
            snapshot
                .steps
                .iter()
                .all(|step| matches!(step, Step::Key { scan_code: 29, .. }))
        );
        assert!(!event(&state, &stop, key(30, true)));
    }
    #[test]
    fn timeout_is_checked_before_any_new_key_and_finished_duration_is_frozen() {
        let state = state();
        state.lock().unwrap().started = Instant::now() - Duration::from_secs(121);
        let stop = Arc::new(Mutex::new(None));
        assert!(event(&state, &stop, key(30, true)));
        let result = snapshot(&state.lock().unwrap());
        assert!(!result.active);
        assert!(result.steps.is_empty());
        state.lock().unwrap().finished_at = Some(Duration::from_millis(1234));
        assert_eq!(snapshot(&state.lock().unwrap()).elapsed_ms, 1234);
    }
    #[test]
    fn injected_keys_escape_and_oem_fn_prefixes_do_not_enter_or_finish_the_recording() {
        let state = state();
        let stop = Arc::new(Mutex::new(None));
        for scan in [30, 1] {
            assert!(!event(
                &state,
                &stop,
                key_events::KeyEvent {
                    injected: true,
                    ..key(scan, true)
                }
            ));
        }
        for scan in [0xe077, 0xe078] {
            assert!(!event(&state, &stop, key(scan, true)));
            assert!(!event(&state, &stop, key(scan, false)));
        }
        let result = snapshot(&state.lock().unwrap());
        assert!(result.active);
        assert!(result.steps.is_empty());
    }
    #[test]
    fn recording_observes_but_never_consumes_modifiers_or_escape() {
        let state = state();
        let stop = Arc::new(Mutex::new(None));
        for key in [key(29, false), key(30, true), key(30, false), key(1, true)] {
            assert!(!observe(&state, &stop, key));
        }
        let result = snapshot(&state.lock().unwrap());
        assert!(!result.active);
        assert_eq!(result.steps.len(), 2);
        assert!(
            result
                .steps
                .iter()
                .all(|step| matches!(step, Step::Key { scan_code: 30, .. }))
        );
    }
    #[test]
    fn polling_or_stopping_without_a_capture_never_initializes_an_input_provider() {
        let controller = Controller::default();
        assert!(controller.read().unwrap().is_none());
        assert!(controller.stop().unwrap().is_none());
        assert!(!controller.active().unwrap());
    }
}
