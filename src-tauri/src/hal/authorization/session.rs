//! 授权在HAL锁外准备；执行与本次恢复复用同一工作进程。
use anyhow::{Context, Result, anyhow, bail};
use std::{
    io::Read,
    sync::mpsc::{self, Receiver, TryRecvError},
    time::Duration,
};
#[path = "events.rs"]
pub(crate) mod events;
#[path = "protocol.rs"]
pub(crate) mod protocol;
pub use events::Binding;
use events::Events;
use protocol::{Phase, Reply};
pub(crate) use protocol::{Task, report_failure, serve};

pub(crate) trait Authorized: Send {
    fn execute(&mut self) -> Result<()>;
    fn rollback(&mut self) -> Result<()>;
    fn dirty(&self) -> bool;
    fn finish(self: Box<Self>) -> Result<()>;
}
pub(crate) struct Session {
    events: Events,
    exit: Option<Receiver<Result<u32>>>,
    directory: Option<tempfile::TempDir>,
    sequence: u64,
    executions: usize,
    execution_limit: usize,
    dirty: bool,
}
impl Session {
    pub(crate) fn prepare(
        binding: Binding,
        directory: tempfile::TempDir,
        arguments: String,
        execution_limit: usize,
        launch: impl FnOnce(&str) -> Result<u32> + Send + 'static,
    ) -> Result<Box<dyn Authorized>> {
        if !(1..=3).contains(&execution_limit) {
            bail!("系统授权执行阶段数量无效");
        }
        let interactive = super::prompt::interactive();
        let mut session = Self {
            events: Events::create(&binding)?,
            exit: None,
            directory: Some(directory),
            sequence: 0,
            executions: 0,
            execution_limit,
            dirty: false,
        };
        let (sender, receiver) = mpsc::channel();
        std::thread::Builder::new()
            .name("control-authorization".into())
            .spawn(move || {
                let _restriction = super::prompt::Restriction::enter(interactive);
                let _ = sender.send(launch(&arguments));
            })?;
        session.exit = Some(receiver);
        loop {
            if session.events.ready.signaled()? {
                return Ok(Box::new(session));
            }
            session.check_exit("系统授权未就绪，未发送执行许可")?;
            std::thread::sleep(Duration::from_millis(15));
        }
    }
    fn check_exit(&mut self, stage: &str) -> Result<()> {
        let receiver = self.exit.as_ref().context("系统授权会话已结束")?;
        match receiver.try_recv() {
            Ok(result) => {
                self.exit.take();
                let detail = self
                    .directory
                    .as_ref()
                    .and_then(|directory| protocol::process_failure(directory.path()));
                self.preserve();
                let code = result?;
                bail!(
                    "{stage}（{code}）：{}",
                    detail.as_deref().unwrap_or("请重新读取，执行结果未确认")
                );
            }
            Err(TryRecvError::Disconnected) => {
                self.exit.take();
                self.preserve();
                bail!("{stage}：系统授权线程意外退出");
            }
            Err(TryRecvError::Empty) => Ok(()),
        }
    }
    fn preserve(&mut self) {
        if let Some(directory) = self.directory.take() {
            let path = directory.keep();
            log::warn!("保留系统授权诊断：{}", path.display());
        }
    }
    fn request(&mut self, phase: Phase) -> Result<()> {
        self.events.response.reset()?;
        self.sequence += 1;
        match phase {
            Phase::Execute => self.events.execute.signal()?,
            Phase::Rollback => self.events.rollback.signal()?,
        }
        loop {
            if self.events.response.signaled()? {
                break;
            }
            self.check_exit("系统授权会话在确认执行结果前退出")?;
            std::thread::sleep(Duration::from_millis(10));
        }
        let path = self
            .directory
            .as_ref()
            .context("系统会话目录已释放")?
            .path()
            .join("response.json");
        let mut bytes = Vec::new();
        std::fs::File::open(path)?
            .take((protocol::MAX_REPLY_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
        let reply = Reply::decode(&bytes, phase, self.sequence)?;
        self.dirty = reply.dirty;
        if let Some(error) = reply.error {
            bail!(error);
        }
        Ok(())
    }
}
impl Authorized for Session {
    fn execute(&mut self) -> Result<()> {
        if self.executions >= self.execution_limit {
            bail!("系统授权会话已执行，不能重复输出");
        }
        self.executions += 1;
        self.dirty = true;
        self.request(Phase::Execute)
    }
    fn rollback(&mut self) -> Result<()> {
        if !self.dirty {
            return Ok(());
        }
        self.request(Phase::Rollback)
    }
    fn dirty(&self) -> bool {
        self.dirty
    }
    fn finish(mut self: Box<Self>) -> Result<()> {
        self.events.finish.signal()?;
        let result = self
            .exit
            .take()
            .context("系统授权会话已结束")?
            .recv()
            .map_err(|_| anyhow!("系统授权线程意外退出"))
            .and_then(|result| result);
        let code = match result {
            Ok(code) => code,
            Err(error) => {
                self.preserve();
                return Err(error.context("系统授权收尾未确认"));
            }
        };
        if code != 0 {
            let detail = self
                .directory
                .as_ref()
                .and_then(|directory| protocol::process_failure(directory.path()));
            self.preserve();
            bail!(
                "系统授权收尾未完成（{code}）：{}",
                detail.as_deref().unwrap_or("请重新读取恢复记录")
            );
        }
        Ok(())
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        if let Some(exit) = self.exit.take() {
            if let Err(error) = self.events.cancel.signal() {
                log::error!("取消系统授权失败：{error:#}");
            }
            if !matches!(exit.recv(), Ok(Ok(0))) {
                self.preserve();
            }
        }
    }
}
