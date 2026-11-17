use super::{Input, MESSAGE_LIMIT, Monitor, Output, Plan};
use crate::display::Mode;
use anyhow::{Result, anyhow, bail};
use std::{
    io::{BufRead, BufReader, Read, Write},
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{self, Receiver},
    time::{Duration, Instant},
};

pub struct Client {
    child: Child,
    input: ChildStdin,
    output: Receiver<Output>,
}
impl Client {
    pub fn spawn(mut command: Command, plan: Plan) -> Result<Self> {
        plan.validate()?;
        command
            .arg("--display-watchdog")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut child = command.spawn()?;
        let input = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("显示监护输入未就绪"))?;
        let reader = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("显示监护输出未就绪"))?;
        let output = read_output(reader);
        let mut this = Self {
            child,
            input,
            output,
        };
        this.send(Input::Prepare { plan })?;
        if let Err(error) = this.expect(|message| matches!(message, Output::Ready)) {
            let _ = this.child.kill();
            let _ = this.child.wait();
            return Err(error);
        }
        this.send(Input::Arm)?;
        this.expect(|message| matches!(message, Output::Armed))?;
        Ok(this)
    }
    fn send(&mut self, message: Input) -> Result<()> {
        writeln!(self.input, "{}", serde_json::to_string(&message)?)?;
        self.input.flush()?;
        Ok(())
    }
    fn expect(&self, wanted: impl FnOnce(&Output) -> bool) -> Result<()> {
        let message = self.output.recv_timeout(Duration::from_secs(3))?;
        if !wanted(&message) {
            bail!("显示监护：{message:?}");
        }
        Ok(())
    }
    fn exited(&mut self) -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(1);
        while Instant::now() < deadline {
            if let Some(status) = self.child.try_wait()? {
                if !status.success() {
                    bail!("显示监护退出异常：{status}");
                }
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        bail!("显示恢复进程尚未释放控制权")
    }
}

fn read_output(reader: impl Read + Send + 'static) -> Receiver<Output> {
    let (sender, output) = mpsc::channel();
    std::thread::spawn(move || {
        let mut reader = BufReader::new(reader);
        loop {
            let mut line = Vec::new();
            let result = reader
                .by_ref()
                .take((MESSAGE_LIMIT + 1) as u64)
                .read_until(b'\n', &mut line);
            if result.is_err() || line.is_empty() || line.len() > MESSAGE_LIMIT {
                break;
            }
            let Ok(message) = serde_json::from_slice::<Output>(&line) else {
                break;
            };
            if sender.send(message).is_err() {
                break;
            }
        }
    });
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn batch_recovery_diagnostics_larger_than_single_display_limit_are_preserved() {
        let message = "隔离诊断".repeat(3000);
        let mut bytes = serde_json::to_vec(&Output::RecoveryFailed {
            message: message.clone(),
        })
        .unwrap();
        bytes.push(b'\n');
        assert!(bytes.len() > 16_384 && bytes.len() <= MESSAGE_LIMIT);
        let received = read_output(std::io::Cursor::new(bytes))
            .recv_timeout(Duration::from_secs(1))
            .unwrap();
        assert!(
            matches!(received, Output::RecoveryFailed { message: actual } if actual == message)
        );
    }
    #[test]
    fn oversized_output_is_bounded_and_does_not_parse_following_reply() {
        let mut bytes = vec![b' '; MESSAGE_LIMIT + 1];
        bytes.extend_from_slice(b"\n{\"event\":\"recovered\"}\n");
        let received =
            read_output(std::io::Cursor::new(bytes)).recv_timeout(Duration::from_secs(1));
        assert!(matches!(
            received,
            Err(mpsc::RecvTimeoutError::Disconnected)
        ));
    }
}
impl Monitor for Client {
    fn keep(&mut self, expected: &[Mode]) -> Result<()> {
        self.send(Input::Keep {
            expected: expected.to_vec(),
        })?;
        self.expect(|message| matches!(message, Output::Kept))?;
        self.exited()
    }
    fn restored(&mut self, expected: &[Mode]) -> Result<()> {
        if self.child.try_wait()?.is_some() {
            return Ok(());
        }
        self.send(Input::Restored {
            expected: expected.to_vec(),
        })?;
        self.expect(|message| matches!(message, Output::Released | Output::Recovered))?;
        self.exited()
    }
    fn recovered(&mut self) -> Result<bool> {
        loop {
            match self.output.try_recv() {
                Ok(Output::Recovered) => {
                    self.exited()?;
                    return Ok(true);
                }
                Ok(message) => bail!("显示监护：{message:?}"),
                Err(mpsc::TryRecvError::Disconnected) => bail!("显示监护管道已断开"),
                Err(mpsc::TryRecvError::Empty) => break,
            }
        }
        if let Some(status) = self.child.try_wait()? {
            bail!("显示监护提前退出：{status}");
        }
        Ok(false)
    }
}
