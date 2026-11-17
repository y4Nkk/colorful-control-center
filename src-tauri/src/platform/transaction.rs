use super::*;

fn send_checked(io: &impl Io, before: &Snapshot, level: u8) -> Result<()> {
    let target = before.target(level)?;
    if target == *before {
        return Ok(());
    }
    io.set_pd(level)?;
    if capture(io)? != target {
        bail!("PD档位或适配器能力回读不一致");
    }
    Ok(())
}
fn recover(io: &impl Io, original: &Snapshot) -> Result<()> {
    let current = capture(io)?;
    current.restorable(original)?;
    send_checked(io, &current, original.level())
}
pub(super) fn apply(
    io: &impl Io,
    baseline: &mut Option<Snapshot>,
    request: &Request,
) -> Result<()> {
    let before = capture(io)?;
    if before.revision() != request.expected_revision {
        bail!("PD档位、适配器或能力已变化，请重新预览");
    }
    Plan::prepare(before, baseline, request.level)?.execute(io, baseline, &mut false)
}

pub(crate) struct Plan {
    before: Snapshot,
    level: u8,
}

impl Plan {
    pub(super) fn prepare(
        before: Snapshot,
        baseline: &Option<Snapshot>,
        level: u8,
    ) -> Result<Self> {
        before.validate_current()?;
        if let Some(initial) = baseline {
            before.same_encoding(initial)?;
        }
        before.target(level)?;
        Ok(Self { before, level })
    }

    pub(super) fn execute(
        &self,
        io: &impl Io,
        baseline: &mut Option<Snapshot>,
        touched: &mut bool,
    ) -> Result<()> {
        self.execute_permitted(io, baseline, touched, &mut || Ok(()))
    }

    pub(super) fn execute_permitted(
        &self,
        io: &impl Io,
        baseline: &mut Option<Snapshot>,
        touched: &mut bool,
        permit: &mut impl FnMut() -> Result<()>,
    ) -> Result<()> {
        if capture(io)? != self.before {
            bail!("PD档位、适配器或能力已变化，请重新预览");
        }
        if self.before.level() == self.level {
            return Ok(());
        }
        permit()?;
        baseline.get_or_insert_with(|| self.before.clone());
        *touched = true;
        if let Err(error) = send_checked(io, &self.before, self.level) {
            let recovery = self.rollback(io);
            *touched = recovery.is_err();
            bail!(ccc_core::msr::with_recovery(
                format!("PD档位写入失败：{error:#}"),
                recovery.map_err(|e| format!("{e:#}"))
            ));
        }
        Ok(())
    }

    pub(super) fn rollback(&self, io: &impl Io) -> Result<()> {
        recover(io, &self.before)
    }
}
pub(super) fn restore(
    io: &impl Io,
    baseline: &Option<Snapshot>,
    expected_revision: &str,
) -> Result<()> {
    let current = capture(io)?;
    if current.revision() != expected_revision {
        bail!("PD恢复预览已变化，请重新读取");
    }
    let initial = baseline.as_ref().context("没有PD初始快照")?;
    current.restorable(initial)?;
    send_checked(io, &current, initial.level())
}
