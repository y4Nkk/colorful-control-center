use super::{
    State,
    worker::{Input, Output},
};
use anyhow::{Context, Result, bail};
use std::{
    io::{BufRead, BufReader, Read, Write},
    process::{Child, Command, Stdio},
    sync::mpsc::{self, Receiver, SyncSender},
    time::Duration,
};

pub(crate) struct Client {
    child: Child,
    input: Option<SyncSender<Input>>,
    output: Receiver<Result<Output, String>>,
    pub operation: String,
    armed: bool,
    finished: bool,
    pub state: Option<State>,
    pub error: Option<String>,
}
impl Client {
    pub fn prepare(operation: String) -> Result<Self> {
        Self::spawn(Command::new(std::env::current_exe()?), operation)
    }
    pub fn spawn(mut command: Command, operation: String) -> Result<Self> {
        command
            .arg("--battery-calibration")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut child = command.spawn()?;
        let writer = child.stdin.take().context("校准输入管道缺失")?;
        let reader = child.stdout.take().context("校准输出管道缺失")?;
        let (sender, output) = mpsc::sync_channel(32);
        std::thread::spawn(move || read(reader, sender));
        let (input, messages) = mpsc::sync_channel(8);
        std::thread::spawn(move || {
            let mut writer = writer;
            for message in messages {
                if serde_json::to_writer(&mut writer, &message).is_err()
                    || writer.write_all(b"\n").is_err()
                    || writer.flush().is_err()
                {
                    break;
                }
            }
        });
        let this = Self {
            child,
            input: Some(input),
            output,
            operation,
            armed: false,
            finished: false,
            state: None,
            error: None,
        };
        this.send(Input::Prepare {
            operation: this.operation.clone(),
        })?;
        match this.output.recv_timeout(Duration::from_secs(3))? {
            Ok(Output::Ready) => Ok(this),
            other => bail!("校准准备失败：{other:?}"),
        }
    }
    fn send(&self, message: Input) -> Result<()> {
        self.input
            .as_ref()
            .context("校准任务正在释放控制")?
            .try_send(message)
            .map_err(|e| anyhow::anyhow!("校准任务输入管道：{e}"))
    }
    pub fn arm(&mut self) -> Result<()> {
        self.armed = true;
        self.send(Input::Arm {
            operation: self.operation.clone(),
        })
    }
    pub fn stop(&self) -> Result<()> {
        self.send(Input::Stop {
            operation: self.operation.clone(),
        })
    }
    pub(super) fn stop_for_exit(&mut self) {
        if self.stop().is_err() {
            self.input.take();
        }
    }
    pub fn pulse(&mut self) -> Result<bool> {
        loop {
            match self.output.try_recv() {
                Ok(Ok(Output::State { state })) => self.state = Some(state),
                Ok(Err(_)) if self.finished => (),
                Ok(Ok(Output::Rejected { message })) | Ok(Err(message)) => {
                    self.error = Some(message);
                    self.input.take();
                }
                Ok(Ok(Output::Finished)) => {
                    self.input.take();
                    self.finished = true;
                }
                Ok(Ok(Output::Ready)) => {
                    self.error = Some("校准任务重复发送准备状态".into());
                    self.input.take();
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.input.take();
                    break;
                }
                Err(mpsc::TryRecvError::Empty) => break,
            }
        }
        if let Some(status) = self.child.try_wait()? {
            if !status.success() {
                self.error = Some(format!("校准任务异常退出：{status}"));
            }
            return Ok(false);
        }
        if self.input.is_some() {
            if let Err(error) = self.send(Input::Heartbeat {
                operation: self.operation.clone(),
            }) {
                self.error = Some(format!("{error:#}"));
                self.input.take();
            }
        }
        Ok(true)
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        self.input.take();
        if !self.armed {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}
fn read(reader: impl Read, sender: SyncSender<Result<Output, String>>) {
    let mut reader = BufReader::new(reader);
    loop {
        let mut line = Vec::new();
        let result = reader.by_ref().take(65_537).read_until(b'\n', &mut line);
        let message = match result {
            Ok(_) if !line.is_empty() && line.len() <= 65_536 => {
                serde_json::from_slice::<Output>(&line)
                    .map_err(|e| format!("校准输出不能解析：{e}"))
            }
            _ => Err("校准状态管道已断开或超出大小限制".into()),
        };
        let failed = message.is_err();
        if sender.send(message).is_err() || failed {
            break;
        }
    }
}
