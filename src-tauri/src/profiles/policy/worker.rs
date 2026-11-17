use super::{
    model::{Binding, Record},
    runtime::Shared,
};
use crate::{
    powerctl::mode::profile::Grant,
    profiles::{
        ProfileFile,
        application::{Application, Origin, Permission},
    },
};
use anyhow::{Context, Result};
use std::{path::Path, sync::Arc};

pub(super) trait ApplicationIo {
    fn begin(
        &self,
        file: &ProfileFile,
        mode: Option<&Grant>,
        origin: Origin,
        permission: Permission,
    ) -> Result<Application>;
    fn current(&self) -> Result<Option<Application>>;
    fn cancel(&self, id: u64) -> Result<Application>;
}
struct Owned {
    binding: Binding,
    id: u64,
    epoch: u64,
    record: Record,
}
#[derive(Default)]
pub(super) struct Worker {
    owned: Option<Owned>,
}
impl Worker {
    pub fn tick(
        &mut self,
        shared: &Arc<Shared>,
        dir: &Path,
        now: u64,
        io: &impl ApplicationIo,
    ) -> Result<()> {
        if self.pending(shared, dir, io)? {
            return Ok(());
        }
        if io.current()?.is_some_and(|report| report.pending.is_some()) {
            return Ok(());
        }
        let Some(work) = shared.take(now)? else {
            return Ok(());
        };
        let file = work
            .binding
            .load(dir)
            .with_context(|| format!("{}档案「{}」", work.trigger.label(), work.binding.profile))?;
        shared.check(work.epoch, &work.record)?;
        let permission = shared.permission(work.epoch, work.record.clone(), work.trigger);
        let report = io
            .begin(
                &file,
                work.binding.mode.as_ref(),
                work.trigger.origin(),
                permission,
            )
            .with_context(|| format!("{}档案「{}」", work.trigger.label(), work.binding.profile))?;
        shared.issue(None);
        if report.pending.is_some() {
            self.owned = Some(Owned {
                binding: work.binding,
                id: report.id,
                epoch: work.epoch,
                record: work.record,
            });
        }
        Ok(())
    }
    fn pending(&mut self, shared: &Shared, dir: &Path, io: &impl ApplicationIo) -> Result<bool> {
        let Some(owned) = &self.owned else {
            return Ok(false);
        };
        let report = io.current()?;
        let Some(report) =
            report.filter(|report| report.id == owned.id && report.pending.is_some())
        else {
            self.owned = None;
            return Ok(false);
        };
        if shared.check(owned.epoch, &owned.record).is_err() || owned.binding.load(dir).is_err() {
            let restored = io.cancel(report.id)?;
            if restored.pending.is_none() {
                self.owned = None;
            }
        }
        Ok(self.owned.is_some())
    }
    pub fn finish(&mut self, io: &impl ApplicationIo) -> Result<()> {
        if let Some(owned) = &self.owned {
            if io
                .current()?
                .is_some_and(|report| report.id == owned.id && report.pending.is_some())
            {
                io.cancel(owned.id)?;
            }
        }
        self.owned = None;
        Ok(())
    }
}
