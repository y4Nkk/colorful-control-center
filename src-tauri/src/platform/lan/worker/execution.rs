use super::*;
use crate::platform::lan::{authorization::Task, recovery::Prepared, transaction};
use std::cell::RefCell;

pub(crate) struct Execution<T> {
    io: T,
    path: PathBuf,
    record: RefCell<store::Record>,
    plans: Vec<transaction::Plan>,
    touched: Vec<transaction::Touched>,
    restoring: bool,
    started: bool,
}
impl<T: transaction::Io> Execution<T> {
    pub fn prepare(
        operation: &Operation,
        frame: &Frame,
        record: store::Record,
        io: T,
    ) -> Result<Self> {
        let plans = operation.plans(frame, &record)?;
        Ok(Self {
            io,
            path: operation.recovery_file.clone(),
            record: RefCell::new(record),
            touched: plans
                .iter()
                .map(|_| transaction::Touched::default())
                .collect(),
            plans,
            restoring: matches!(operation.intent, Intent::Restore { .. }),
            started: false,
        })
    }
    fn journal(&self) -> Prepared<'_, T> {
        Prepared {
            io: &self.io,
            path: &self.path,
            record: &self.record,
        }
    }
    fn report(&self, result: Result<()>) -> Result<()> {
        let recorded = self
            .journal()
            .report(result.as_ref().err().map(|error| format!("{error:#}")));
        match (result, recorded) {
            (Err(error), Err(saved)) => bail!("{error:#}；保存操作诊断失败：{saved:#}"),
            (Err(error), _) => Err(error),
            (_, result) => result,
        }
    }
    fn undo(&mut self, end: usize) -> Result<()> {
        let mut failures = Vec::new();
        for index in (0..end).rev() {
            let journal = Prepared {
                io: &self.io,
                path: &self.path,
                record: &self.record,
            };
            if let Err(error) =
                transaction::rollback(&journal, &self.plans[index], &mut self.touched[index])
            {
                failures.push(format!(
                    "{}：{error:#}",
                    self.plans[index].expected.identity.instance
                ));
            }
        }
        if !failures.is_empty() {
            bail!(failures.join("；"));
        }
        Ok(())
    }
}
impl<T: transaction::Io + Send> Task for Execution<T> {
    fn execute(&mut self) -> Result<()> {
        if self.started {
            bail!("LAN授权会话已执行，不能重复输出");
        }
        self.started = true;
        let mut failures = Vec::new();
        for index in 0..self.plans.len() {
            let journal = Prepared {
                io: &self.io,
                path: &self.path,
                record: &self.record,
            };
            let result = if self.restoring {
                transaction::restore(&journal, &self.plans[index], &mut self.touched[index])
            } else {
                transaction::execute(&journal, &self.plans[index], &mut self.touched[index])
            };
            if let Err(error) = result {
                failures.push(format!("{error:#}"));
                if !self.restoring {
                    if let Err(recovery) = self.undo(index) {
                        failures.push(format!("恢复先前网卡：{recovery:#}"));
                    }
                    break;
                }
            }
        }
        let result = if failures.is_empty() {
            Ok(())
        } else {
            Err(anyhow::anyhow!(failures.join("；")))
        };
        self.report(result)
    }
    fn rollback(&mut self) -> Result<()> {
        let result = self.undo(self.plans.len());
        self.report(result)
    }
    fn dirty(&self) -> bool {
        self.touched.iter().any(transaction::Touched::dirty)
    }
}
