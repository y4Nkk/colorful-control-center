//! 一个工作进程拥有完整冻结序列，进程退出仍按阶段逆序补偿。
use super::request::{Invocation, Request};
use crate::hal::authorization::session::Task;
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Sequence {
    pub requests: Vec<Request>,
}
impl Sequence {
    pub fn new(requests: &[Request]) -> Result<Self> {
        let sequence = Self {
            requests: requests.to_vec(),
        };
        sequence.encoded()?;
        Ok(sequence)
    }
    pub fn encoded(&self) -> Result<Vec<u8>> {
        if self.requests.is_empty() || self.requests.len() > 3 {
            bail!("独显阶段序列长度无效");
        }
        for request in &self.requests {
            request.encoded()?;
            if !request.restore && request.expected.target() == request.target {
                bail!("独显序列不接受无变化阶段");
            }
        }
        for pair in self.requests.windows(2) {
            let expected = pair[0].expected.projected(pair[0].target);
            if pair[1].expected != expected {
                bail!("独显阶段的设备或前态不连续");
            }
        }
        let bytes = serde_json::to_vec(self)?;
        if bytes.len() > super::request::LIMIT {
            bail!("独显阶段序列过大");
        }
        Ok(bytes)
    }
}

pub(super) struct Execution {
    invocation: Invocation,
    sequence: Sequence,
    tasks: Vec<Box<dyn Task>>,
    prepare: Box<dyn FnMut(Request) -> Result<Box<dyn Task>> + Send>,
    attempted: usize,
    stopped: bool,
}
impl Execution {
    pub fn new(
        invocation: Invocation,
        prepare: impl FnMut(Request) -> Result<Box<dyn Task>> + Send + 'static,
    ) -> Result<Self> {
        Ok(Self {
            sequence: invocation.request()?,
            invocation,
            tasks: Vec::new(),
            prepare: Box::new(prepare),
            attempted: 0,
            stopped: false,
        })
    }
    fn next(&mut self) -> Result<()> {
        if self.stopped || self.attempted >= self.sequence.requests.len() {
            bail!("独显阶段序列已结束，不能继续输出");
        }
        if self
            .invocation
            .request()
            .context("独显阶段许可后的请求复核失败")?
            != self.sequence
        {
            bail!("独显阶段序列已变化");
        }
        let request = self.sequence.requests[self.attempted].clone();
        self.attempted += 1;
        self.tasks.push(
            (self.prepare)(request).with_context(|| format!("独显阶段{}准备", self.attempted))?,
        );
        self.tasks
            .last_mut()
            .unwrap()
            .execute()
            .with_context(|| format!("独显阶段{}执行", self.attempted))
    }
}
impl Task for Execution {
    fn executions(&self) -> usize {
        self.sequence.requests.len()
    }
    fn execute(&mut self) -> Result<()> {
        let result = self.next();
        if result.is_err() {
            self.stopped = true;
        }
        result
    }
    fn rollback(&mut self) -> Result<()> {
        self.stopped = true;
        let mut errors = Vec::new();
        for (index, task) in self
            .tasks
            .iter_mut()
            .enumerate()
            .rev()
            .filter(|(_, task)| task.dirty())
        {
            if let Err(error) = task.rollback() {
                errors.push(format!("独显阶段{}：{error:#}", index + 1));
            }
        }
        if !errors.is_empty() {
            bail!(errors.join("；"));
        }
        Ok(())
    }
    fn dirty(&self) -> bool {
        self.tasks.iter().any(|task| task.dirty())
    }
    fn finish(&mut self) -> Result<()> {
        if self.dirty() && (self.stopped || self.attempted != self.sequence.requests.len()) {
            bail!("独显序列尚未完整执行，不能提交部分目标");
        }
        Ok(())
    }
}
