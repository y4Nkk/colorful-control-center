use super::*;

pub(super) struct Plan {
    before: ChargeState,
    target: ChargeState,
}
impl Plan {
    pub fn profile(
        before: ChargeState,
        baseline: &Option<ChargeState>,
        target: &ChargeTarget,
    ) -> Result<Self> {
        target.validate()?;
        if let Some(initial) = baseline {
            before.same_capabilities(initial)?;
        }
        Self::restore(before, &target.state())
    }
    pub fn prepare(
        before: ChargeState,
        baseline: &Option<ChargeState>,
        values: &ChargeValues,
    ) -> Result<Self> {
        if let Some(initial) = baseline {
            before.same_capabilities(initial)?;
        }
        let target = before.target(values)?;
        Ok(Self { before, target })
    }
    pub fn restore(before: ChargeState, initial: &ChargeState) -> Result<Self> {
        before.same_capabilities(initial)?;
        Ok(Self {
            before,
            target: initial.clone(),
        })
    }
    pub fn execute(
        &self,
        io: &dyn Io,
        baseline: &mut Option<ChargeState>,
        touched: &mut bool,
    ) -> Result<()> {
        if io.power()?.present != Some(true) {
            bail!("未确认电池存在，未修改充电保护");
        }
        if current(io)? != self.before {
            bail!("充电状态或候选已变化，请重新读取并确认");
        }
        if self.before == self.target {
            return Ok(());
        }
        baseline.get_or_insert_with(|| self.before.clone());
        *touched = true;
        if let Err(error) = write(io, &self.target) {
            let recovered = self.rollback(io);
            *touched = recovered.is_err();
            bail!(ccc_core::msr::with_recovery(
                format!("充电设置失败：{error:#}"),
                recovered.map_err(|error| format!("{error:#}"))
            ));
        }
        Ok(())
    }
    pub fn rollback(&self, io: &dyn Io) -> Result<()> {
        if io.power()?.present != Some(true) {
            bail!("未确认电池存在，未恢复充电保护");
        }
        write(io, &self.before)
    }
}
