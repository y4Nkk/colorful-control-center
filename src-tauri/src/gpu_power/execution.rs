use super::{
    operation::{self, Io},
    request::Request,
};
use crate::hal::authorization::session::Task;
use anyhow::{Result, anyhow, bail};

pub(crate) struct Execution<T: Io + Send> {
    io: T,
    request: Request,
    dirty: bool,
    uncertain: bool,
}
impl<T: Io + Send> Execution<T> {
    pub fn new(io: T, request: Request) -> Self {
        Self {
            io,
            request,
            dirty: false,
            uncertain: false,
        }
    }
}
impl<T: Io + Send> Task for Execution<T> {
    fn execute(&mut self) -> Result<()> {
        let before = self.io.inspect(&self.request.expected.id)?;
        if before != self.request.expected {
            bail!("独显完整设备事实已变化，未执行目标");
        }
        self.dirty = self.request.restore || before.target() != self.request.target;
        let result = if self.request.restore {
            operation::replay(&self.io, &self.request.plan())
        } else {
            operation::execute(&self.io, &self.request.plan())
        };
        if let Err(error) = result {
            self.dirty = error.recovery_failed;
            self.uncertain = error.recovery_failed;
            bail!(
                "{}：{}",
                operation::Failure::describe(error.exit_code()),
                error.message
            );
        }
        Ok(())
    }
    fn rollback(&mut self) -> Result<()> {
        if !self.dirty {
            return Ok(());
        }
        let current = self.io.inspect(&self.request.expected.id)?;
        if current.id != self.request.expected.id || !current.healthy() {
            bail!("独显恢复设备不存在或存在故障");
        }
        let before = self.request.expected.target();
        if current.target() == before && !self.uncertain {
            self.dirty = false;
            return Ok(());
        }
        if current.target() != self.request.target && current.target() != before {
            bail!("独显状态被外部修改，拒绝覆盖本次目标以外的状态");
        }
        let plan = super::Plan {
            id: current.id.clone(),
            expected: current.target(),
            target: before,
        };
        operation::replay(&self.io, &plan).map_err(|e| anyhow!("{}", e.message))?;
        self.dirty = false;
        self.uncertain = false;
        Ok(())
    }
    fn dirty(&self) -> bool {
        self.dirty
    }
}
