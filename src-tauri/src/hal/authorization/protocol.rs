use super::{
    Binding,
    events::{Event, Events},
};
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use windows_sys::Win32::{Foundation::*, System::Threading::*};

pub const MAX_REPLY_BYTES: usize = 256 * 1024;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProcessFailure {
    error: String,
}
pub(crate) fn report_failure(directory: &std::path::Path, error: &str) -> Result<()> {
    let bytes = serde_json::to_vec(&ProcessFailure {
        error: error.into(),
    })?;
    if bytes.len() > MAX_REPLY_BYTES {
        bail!("系统进程诊断过大");
    }
    crate::storage::write(&directory.join("process-error.json"), &bytes)?;
    Ok(())
}
pub(crate) fn process_failure(directory: &std::path::Path) -> Option<String> {
    use std::io::Read;
    let mut bytes = Vec::new();
    std::fs::File::open(directory.join("process-error.json"))
        .ok()?
        .take((MAX_REPLY_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() > MAX_REPLY_BYTES {
        return None;
    }
    serde_json::from_slice::<ProcessFailure>(&bytes)
        .ok()
        .map(|failure| failure.error)
}
pub trait Task: Send {
    fn executions(&self) -> usize {
        1
    }
    fn execute(&mut self) -> Result<()>;
    fn rollback(&mut self) -> Result<()>;
    fn dirty(&self) -> bool;
    fn finish(&mut self) -> Result<()> {
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Phase {
    Execute,
    Rollback,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Reply {
    pub phase: Phase,
    pub sequence: u64,
    pub dirty: bool,
    pub error: Option<String>,
}
impl Reply {
    pub fn encoded(&self) -> Result<Vec<u8>> {
        let bytes = serde_json::to_vec(self)?;
        if bytes.len() > MAX_REPLY_BYTES {
            bail!("系统授权响应过大");
        }
        Ok(bytes)
    }
    pub fn decode(bytes: &[u8], phase: Phase, sequence: u64) -> Result<Self> {
        if bytes.len() > MAX_REPLY_BYTES {
            bail!("系统授权响应过大");
        }
        let reply: Self = serde_json::from_slice(bytes)?;
        if reply.phase != phase || reply.sequence != sequence {
            bail!("系统授权响应阶段或序号不一致");
        }
        Ok(reply)
    }
}
pub(crate) fn serve(
    binding: &Binding,
    directory: &std::path::Path,
    prepare: impl FnOnce() -> Result<Box<dyn Task>>,
) -> Result<()> {
    binding.validate()?;
    let events = Events::open(binding)?;
    let parent = Event::parent(binding.parent_id)?;
    let mut task: Option<Box<dyn Task>> = None;
    let result = run(directory, &events, &parent, prepare, &mut task);
    if let Err(error) = result {
        let recovered = task
            .as_mut()
            .filter(|task| task.dirty())
            .map(|task| task.rollback())
            .transpose();
        if let Err(recovery) = recovered {
            bail!("{error:#}；系统会话退出补偿失败：{recovery:#}");
        }
        return Err(error);
    }
    Ok(())
}
fn run(
    directory: &std::path::Path,
    events: &Events,
    parent: &Event,
    prepare: impl FnOnce() -> Result<Box<dyn Task>>,
    task: &mut Option<Box<dyn Task>>,
) -> Result<()> {
    let mut prepare = Some(prepare);
    let (mut executed, mut sequence) = (0, 0);
    events.ready.signal()?;
    loop {
        let handles = [
            events.cancel.raw(),
            parent.raw(),
            events.finish.raw(),
            events.rollback.raw(),
            events.execute.raw(),
        ];
        // 就绪后仍可能等待其他设备准备；由取消或父进程退出结束等待。
        let index =
            unsafe { WaitForMultipleObjects(handles.len() as u32, handles.as_ptr(), 0, INFINITE) };
        match index {
            WAIT_OBJECT_0 => {
                if let Some(task) = task.as_mut().filter(|task| task.dirty()) {
                    task.rollback()?;
                }
                return Ok(());
            }
            code if code == WAIT_OBJECT_0 + 1 => {
                if let Some(task) = task.as_mut().filter(|task| task.dirty()) {
                    task.rollback()?;
                }
                return Ok(());
            }
            code if code == WAIT_OBJECT_0 + 2 => {
                return task.as_mut().map(|task| task.finish()).unwrap_or(Ok(()));
            }
            code if code == WAIT_OBJECT_0 + 3 || code == WAIT_OBJECT_0 + 4 => {
                let phase = if code == WAIT_OBJECT_0 + 3 {
                    Phase::Rollback
                } else {
                    Phase::Execute
                };
                match phase {
                    Phase::Execute => events.execute.reset()?,
                    Phase::Rollback => events.rollback.reset()?,
                }
                sequence += 1;
                let result = match phase {
                    Phase::Execute => execute(task, &mut prepare, &mut executed),
                    Phase::Rollback => task.as_mut().map(|task| task.rollback()).unwrap_or(Ok(())),
                };
                let reply = Reply {
                    phase,
                    sequence,
                    dirty: task.as_ref().is_some_and(|task| task.dirty()),
                    error: result.err().map(|error| format!("{error:#}")),
                };
                crate::storage::write(&directory.join("response.json"), &reply.encoded()?)?;
                events.response.signal()?;
            }
            _ => return Err(std::io::Error::last_os_error().into()),
        }
    }
}
fn execute(
    task: &mut Option<Box<dyn Task>>,
    prepare: &mut Option<impl FnOnce() -> Result<Box<dyn Task>>>,
    executed: &mut usize,
) -> Result<()> {
    if task.is_none() {
        let create = prepare
            .take()
            .ok_or_else(|| anyhow::anyhow!("系统授权任务准备已失败"))?;
        *task = Some(create()?);
    }
    let task = task.as_mut().unwrap();
    let limit = task.executions();
    if !(1..=3).contains(&limit) || *executed >= limit {
        bail!("系统会话已执行，拒绝重复输出");
    }
    *executed += 1;
    task.execute()
}
