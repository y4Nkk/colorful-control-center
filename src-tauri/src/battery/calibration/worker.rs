//! 独立任务协议；ARM前禁止控制写入，失联后恢复，不自动接续中断的校准。
use super::{
    Access, State, Status, Task,
    store::{Record, Store},
};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    io::{BufRead, BufReader, Read},
    sync::mpsc::{self, Receiver},
    time::{Duration, Instant},
};

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "camelCase", deny_unknown_fields)]
pub(crate) enum Input {
    Prepare { operation: String },
    Arm { operation: String },
    Heartbeat { operation: String },
    Stop { operation: String },
    RetryRestore { operation: String },
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "camelCase", deny_unknown_fields)]
pub(crate) enum Output {
    Ready,
    State { state: State },
    Rejected { message: String },
    Finished,
}
pub(crate) fn read(reader: impl Read + Send + 'static) -> Receiver<Input> {
    let (sender, receiver) = mpsc::sync_channel(16);
    std::thread::spawn(move || {
        let mut reader = BufReader::new(reader);
        loop {
            let mut line = Vec::new();
            let result = reader.by_ref().take(65_537).read_until(b'\n', &mut line);
            if result.is_err() || line.is_empty() || line.len() > 65_536 {
                break;
            }
            let Ok(message) = serde_json::from_slice::<Input>(&line) else {
                break;
            };
            if sender.send(message).is_err() {
                break;
            }
        }
    });
    receiver
}
fn prepare(input: &Receiver<Input>, io: &mut dyn Access, store: &dyn Store) -> Result<Record> {
    let operation = match input.recv_timeout(Duration::from_secs(3))? {
        Input::Prepare { operation } => operation,
        _ => bail!("校准任务缺少准备请求"),
    };
    let record = store.load()?.context("校准恢复记录尚未准备")?;
    record.validate()?;
    if record.operation != operation
        || record.state.is_some()
        || !record.pending
        || record.outcome.is_some()
        || record.armed
    {
        bail!("校准任务与已准备记录不一致，未接管控制");
    }
    io.ready()?;
    io.facts()?.before_start()?;
    io.verify_baseline(&record.baseline)?;
    Ok(record)
}
pub(crate) fn run(
    input: Receiver<Input>,
    io: &mut dyn Access,
    store: &dyn Store,
    mut output: impl FnMut(Output) -> Result<()>,
) -> Result<()> {
    let mut record = prepare(&input, io, store)?;
    output(Output::Ready)?;
    let armed = matches!(input.recv_timeout(Duration::from_secs(3)),
        Ok(Input::Arm { operation }) if operation == record.operation);
    if !armed {
        return Ok(());
    }
    record.armed = true;
    store.save(&record)?;
    let Some(mut running) = Running::start(io, store, record, &mut output)? else {
        return Ok(());
    };
    loop {
        running.report(io, store, &mut output);
        if running.stored && !running.task.state.active() && !running.task.state.recovery_required()
        {
            let _ = output(Output::Finished);
            return Ok(());
        }
        running.receive(&input, io);
        running.advance(io, store);
    }
}
struct Running {
    task: Task,
    record: Record,
    started: Instant,
    last_heartbeat: Instant,
    last_tick: Instant,
    disconnected: bool,
    stored: bool,
    save_error: Option<String>,
}
impl Running {
    fn start(
        io: &mut dyn Access,
        store: &dyn Store,
        mut record: Record,
        output: &mut impl FnMut(Output) -> Result<()>,
    ) -> Result<Option<Self>> {
        let started = Instant::now();
        let baseline = record.baseline.clone();
        let task = match Task::start(io, baseline, Duration::ZERO, &mut || {
            record.plan_prepared = true;
            store.save(&record)
        }) {
            Ok(task) => task,
            Err(error) => {
                record.pending = false;
                record.armed = false;
                record.outcome = Some(Status::Failed);
                record.error = Some(format!("校准接管前检查失败：{error:#}"));
                store.save(&record)?;
                output(Output::Rejected {
                    message: record.error.unwrap(),
                })?;
                return Ok(None);
            }
        };
        let mut this = Self {
            task,
            record,
            started,
            last_heartbeat: started,
            last_tick: started,
            disconnected: false,
            stored: false,
            save_error: None,
        };
        this.persist(io, store);
        Ok(Some(this))
    }
    fn report(
        &mut self,
        io: &mut dyn Access,
        store: &dyn Store,
        output: &mut impl FnMut(Output) -> Result<()>,
    ) {
        let result = output(Output::State {
            state: self.task.state.clone(),
        });
        if let Err(error) = result {
            if self.task.state.active() {
                self.task.fail(io, format!("校准状态管道断开：{error:#}"));
                self.persist(io, store);
            }
        }
        if let Some(message) = &self.save_error {
            let _ = output(Output::Rejected {
                message: message.clone(),
            });
        }
    }
    fn receive(&mut self, input: &Receiver<Input>, io: &mut dyn Access) {
        match input.recv_timeout(Duration::from_millis(250)) {
            Ok(Input::Heartbeat { operation }) if operation == self.record.operation => {
                self.last_heartbeat = Instant::now();
            }
            Ok(Input::Stop { operation }) if operation == self.record.operation => {
                self.task.stop(io)
            }
            Ok(Input::RetryRestore { operation })
                if operation == self.record.operation && self.task.state.recovery_required() =>
            {
                self.task.retry_restore(io)
            }
            Err(mpsc::RecvTimeoutError::Timeout) => (),
            Err(mpsc::RecvTimeoutError::Disconnected) => self.disconnected = true,
            _ => self.task.fail(io, "校准任务收到不匹配的控制请求".into()),
        }
        if self.task.state.active()
            && (self.disconnected || self.last_heartbeat.elapsed() > Duration::from_secs(3))
        {
            self.task.fail(io, "主控制进程已断开或心跳停止".into());
        }
        if self.disconnected {
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    fn advance(&mut self, io: &mut dyn Access, store: &dyn Store) {
        if self.last_tick.elapsed() >= Duration::from_secs(1) {
            self.last_tick = Instant::now();
            if self.task.state.recovery_required() {
                self.task.retry_restore(io);
            } else {
                self.task.tick(io, self.started.elapsed());
            }
            self.persist(io, store);
        } else if !self.task.state.active() {
            self.persist(io, store);
        }
    }
    fn persist(&mut self, io: &mut dyn Access, store: &dyn Store) {
        self.record.update(&self.task);
        let mut result = store.save(&self.record);
        if let Err(error) = &result {
            if self.task.state.active() {
                self.task.fail(io, format!("校准状态保存失败：{error:#}"));
                self.record.update(&self.task);
                result = store.save(&self.record);
            }
        }
        self.stored = result.is_ok();
        self.save_error = result.err().map(|e| format!("校准恢复记录保存失败：{e:#}"));
    }
}
