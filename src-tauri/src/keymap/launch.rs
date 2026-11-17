//! 程序宏只消费已应用配置的过滤驱动System事件，不执行XML或草稿中的任意事件。
#[cfg(not(test))]
#[path = "launch/native.rs"]
mod native;
#[path = "launch/runtime.rs"]
mod runtime;
#[cfg(test)]
#[path = "launch/test_support.rs"]
mod test_support;
#[cfg(test)]
#[path = "launch/tests.rs"]
mod tests;
use super::{
    model::{Phase, Record},
    recording,
};
use crate::hal::event_log;
use anyhow::{Context, Result, anyhow, bail};
use ccc_core::keymap::{Action, Device};
use serde::Serialize;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
};
#[cfg(test)]
pub(crate) use test_support::TestPrograms;

#[derive(Clone)]
struct Program {
    device: Device,
    event_id: u16,
    path: String,
}
fn programs(record: &Record) -> Vec<Program> {
    record
        .current
        .devices
        .iter()
        .filter(|group| group.enabled && record.owned.contains(&group.device))
        .flat_map(|group| {
            group
                .bindings
                .iter()
                .filter_map(move |binding| match &binding.action {
                    Action::Launch { path, event_id } => Some(Program {
                        device: group.device,
                        event_id: *event_id,
                        path: path.clone(),
                    }),
                    _ => None,
                })
        })
        .collect()
}
pub(super) fn required(record: &Record) -> bool {
    !programs(record).is_empty()
}
pub(super) fn program_scope(record: &Record) -> Vec<Device> {
    let mut devices: Vec<_> = programs(record)
        .into_iter()
        .map(|program| program.device)
        .collect();
    devices.sort();
    devices.dedup();
    devices
}
fn select<'a>(programs: &'a [Program], event: &event_log::Record) -> Option<&'a str> {
    if event.event_type != 4 || event.event_id & 0xffff0000 != 0x40000000 {
        return None;
    }
    let device = if event.source.eq_ignore_ascii_case("HKKbdFltr") {
        Device::Keyboard
    } else if event.source.eq_ignore_ascii_case("HKMouFltr") {
        Device::Mouse
    } else {
        return None;
    };
    programs
        .iter()
        .find(|program| {
            program.device == device && program.event_id as u32 == event.event_id & 0xffff
        })
        .map(|program| program.path.as_str())
}
fn authorize(actual: Option<&Record>, expected: &Record) -> Result<bool> {
    if expected.phase != Phase::Applied {
        bail!("程序监听目标不是已应用配置");
    }
    if actual == Some(expected) {
        return Ok(true);
    }
    let mut pending = expected.clone();
    pending.phase = Phase::Prepared;
    if actual == Some(&pending) {
        return Ok(false);
    }
    bail!("映射保存记录已变化，程序监听已停止，请重新预览并启用监听")
}
fn dispatch(
    event: &event_log::Record,
    programs: &[Program],
    expected: &Record,
    recording: impl FnOnce() -> Result<bool>,
    read: impl FnOnce() -> Result<Option<Record>>,
    launch: impl FnOnce(&str) -> Result<()>,
) -> Result<bool> {
    let Some(program) = select(programs, event) else {
        return Ok(false);
    };
    if recording()? || !authorize(read()?.as_ref(), expected)? {
        return Ok(false);
    }
    launch(program)?;
    Ok(true)
}
#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub running: bool,
    pub error: Option<String>,
}
pub(crate) struct Session {
    control: Arc<runtime::Control>,
    thread: Option<thread::JoinHandle<()>>,
    record: Record,
}
impl Session {
    pub fn ready(&self) -> Result<()> {
        if self.control.stop.load(std::sync::atomic::Ordering::SeqCst)
            || self
                .thread
                .as_ref()
                .is_some_and(|thread| thread.is_finished())
        {
            bail!(
                "程序事件会话已停止：{}",
                self.status()?
                    .error
                    .unwrap_or_else(|| "输出线程不可用".into())
            );
        }
        Ok(())
    }
    pub fn matches(&self, record: &Record) -> bool {
        self.record == *record
    }
    pub fn pause(&self) {
        self.control.pause();
    }
    pub fn activate(&self) -> Result<()> {
        self.ready()?;
        self.control
            .epoch
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.control
            .active
            .store(true, std::sync::atomic::Ordering::SeqCst);
        if let Some(thread) = &self.thread {
            thread.thread().unpark();
        }
        Ok(())
    }
    fn enabled(&self) -> bool {
        self.control
            .accepts(self.control.epoch.load(std::sync::atomic::Ordering::SeqCst))
    }
    fn status(&self) -> Result<Status> {
        Ok(self
            .control
            .state
            .lock()
            .map_err(|_| anyhow!("程序监听状态异常"))?
            .clone())
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        self.control
            .stop
            .store(true, std::sync::atomic::Ordering::SeqCst);
        self.control.pause();
        if let Some(thread) = self.thread.take() {
            thread.thread().unpark();
            let _ = thread.join();
        }
    }
}
#[derive(Default)]
pub struct Controller {
    session: Mutex<Option<Session>>,
}
impl Controller {
    pub fn status(&self) -> Result<Status> {
        let session = self
            .session
            .lock()
            .map_err(|_| anyhow!("程序监听状态异常"))?;
        session
            .as_ref()
            .map_or_else(|| Ok(Status::default()), Session::status)
    }
    pub fn enabled(&self) -> Result<bool> {
        Ok(self
            .session
            .lock()
            .map_err(|_| anyhow!("程序监听状态异常"))?
            .as_ref()
            .is_some_and(Session::enabled))
    }
    pub fn pause(&self) -> Result<bool> {
        let session = self
            .session
            .lock()
            .map_err(|_| anyhow!("程序监听状态异常"))?;
        let was_enabled = session.as_ref().is_some_and(Session::enabled);
        if let Some(session) = session.as_ref() {
            session.control.pause();
        }
        Ok(was_enabled)
    }
    pub fn resume(&self) -> Result<()> {
        self.session
            .lock()
            .map_err(|_| anyhow!("程序监听状态异常"))?
            .as_ref()
            .context("原程序监听会话已丢失")?
            .activate()
    }
    pub(super) fn replace(&self, candidate: Option<Session>) -> Result<()> {
        let mut state = self
            .session
            .lock()
            .map_err(|_| anyhow!("程序监听状态异常"))?;
        let previous = std::mem::replace(&mut *state, candidate);
        drop(state);
        drop(previous);
        Ok(())
    }
}
pub(super) fn prepare(
    path: PathBuf,
    record: Record,
    recording: Arc<recording::Controller>,
    gate: Arc<Mutex<()>>,
) -> Result<Option<Session>> {
    let programs = programs(&record);
    if programs.is_empty() {
        return Ok(None);
    }
    #[cfg(not(test))]
    {
        Ok(Some(native::prepare(
            path, record, programs, recording, gate,
        )?))
    }
    #[cfg(test)]
    {
        let _ = (path, record, programs, recording, gate);
        bail!("隔离测试未注入程序事件工厂，未读取真实日志或启动程序")
    }
}
