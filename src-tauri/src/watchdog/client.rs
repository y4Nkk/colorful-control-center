//! 父进程管道客户端；应用与隔离进程测试共用同一握手与存活判断。
use super::protocol::read_lines;
use anyhow::{Context, Result, anyhow, bail};
use std::{
    io::Write,
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{self, Receiver},
    time::Duration,
};

pub struct Watchdog {
    child: Child,
    input: Option<ChildStdin>,
    output: Receiver<String>,
}

impl Watchdog {
    pub fn start() -> Result<Self> {
        Self::spawn(Command::new(std::env::current_exe()?))
    }

    pub fn spawn(mut command: Command) -> Result<Self> {
        command.arg("--fan-watchdog");
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut child = command.spawn().context("启动风扇监护失败")?;
        let input = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("监护输入管道不可用"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("监护输出管道不可用"))?;
        let output = read_lines(stdout);
        let mut this = Self {
            child,
            input: Some(input),
            output,
        };
        if let Err(error) = this.expect("READY") {
            // 尚未发送 ARM，不承担硬件恢复责任；结束未就绪子进程，避免握手超时遗留。
            let stopped = this.child.try_wait().and_then(|status| {
                if status.is_none() {
                    this.child.kill()?;
                    this.child.wait()?;
                }
                Ok(())
            });
            return match stopped {
                Ok(()) => Err(error),
                Err(stop) => Err(anyhow!("{error:#}；结束未就绪监护失败：{stop}")),
            };
        }
        Ok(this)
    }

    pub fn arm(&mut self) -> Result<()> {
        self.send("ARM")?;
        self.expect("ARMED")
    }

    fn send(&mut self, value: &str) -> Result<()> {
        let input = self
            .input
            .as_mut()
            .ok_or_else(|| anyhow!("风扇监护正在恢复"))?;
        writeln!(input, "{value}").context("风扇监护连接中断")?;
        input.flush()?;
        Ok(())
    }

    fn expect(&self, wanted: &str) -> Result<()> {
        match self.output.recv_timeout(Duration::from_secs(3)) {
            Ok(line) if line == wanted => Ok(()),
            Ok(line) => bail!(
                "风扇监护：{}",
                line.strip_prefix("FAILED:").unwrap_or(&line)
            ),
            Err(error) => bail!("风扇监护握手失败：{error}"),
        }
    }

    pub fn pulse(&mut self) -> Result<()> {
        let disconnected = match self.output.try_recv() {
            Ok(message) => bail!("风扇监护：{message}"),
            Err(mpsc::TryRecvError::Disconnected) => true,
            Err(mpsc::TryRecvError::Empty) => false,
        };
        if let Some(status) = self.child.try_wait()? {
            // 进程退出与读取线程投递末条诊断之间可能有短暂竞态。
            if let Ok(message) = self.output.recv_timeout(Duration::from_millis(100)) {
                bail!("风扇监护：{message}（{status}）");
            }
            bail!("风扇监护已退出：{status}");
        }
        if disconnected {
            bail!("风扇监护输出连接中断");
        }
        self.send("HB")
    }

    pub fn stop_heartbeats(&mut self) {
        self.input.take();
    }

    pub fn exited(&mut self) -> Result<bool> {
        Ok(self.child.try_wait()?.is_some())
    }

    pub fn disarm(&mut self) -> Result<()> {
        if self.exited()? {
            bail!("风扇监护已退出，尚未确认风扇已退出软件调速");
        }
        self.send("DISARM")?;
        self.expect("DISARMED")?;
        for _ in 0..30 {
            if self.child.try_wait()?.is_some() {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        bail!("监护进程未按期退出")
    }
}
