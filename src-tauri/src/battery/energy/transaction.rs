use super::*;

pub(crate) struct Plan {
    pub(in crate::battery) before: Snapshot,
    pub(in crate::battery) previous: Option<Record>,
    policy: Policy,
    pub(in crate::battery) charge: ChargeState,
}
#[derive(Default)]
pub(crate) struct Change {
    saved: Option<Record>,
    dirty: bool,
}
impl Change {
    pub fn dirty(&self) -> bool {
        self.dirty
    }
}
impl Plan {
    pub(in crate::battery) fn prepare(
        io: &dyn Io,
        store: &dyn Store,
        policy: &Policy,
    ) -> Result<Self> {
        if !io.present()? {
            bail!("未确认电池存在，未修改节能计划");
        }
        let (before, previous) = observe(io, store)?.context("本机未提供节能计划能力")?;
        if previous
            .as_ref()
            .is_some_and(|record| record.pending.is_some())
        {
            bail!("上次节能操作尚需恢复，不能继续应用");
        }
        if let Some(record) = &previous {
            before.charge.same_capabilities(&record.baseline.charge)?;
            before.charge.same_capabilities(&record.stop_charge)?;
        }
        policy.validate(&before.capability)?;
        let mut charge = previous.as_ref().map_or_else(
            || before.charge.clone(),
            |record| record.stop_charge.clone(),
        );
        if policy.enabled {
            charge = before.charge.clone();
            charge.current = super::super::model::ChargeValues {
                enabled: true,
                start: policy.limits.floor,
                stop: policy.limits.ceiling,
            };
        }
        before.charge.same_capabilities(&charge)?;
        Ok(Self {
            before,
            previous,
            policy: policy.clone(),
            charge,
        })
    }
    fn record_before(&self) -> Record {
        self.previous.clone().unwrap_or(Record {
            version: 1,
            policy: self.before.policy.clone(),
            capability: self.before.capability.clone(),
            baseline: self.before.clone(),
            stop_charge: self.before.charge.clone(),
            pending: None,
            last_error: None,
        })
    }
    pub(in crate::battery) fn execute(
        &self,
        io: &dyn Io,
        store: &dyn Store,
        change: &mut Change,
    ) -> Result<()> {
        if !io.present()? {
            bail!("未确认电池存在，未修改节能计划");
        }
        let (current, record) = observe(io, store)?.context("节能能力已丢失")?;
        if current != self.before || record != self.previous {
            bail!("节能计划或充电策略已变化，请重新预览");
        }
        let mut prepared = self.record_before();
        if self.policy.enabled
            && !self
                .before
                .policy
                .as_ref()
                .is_some_and(|policy| policy.enabled)
        {
            prepared.stop_charge = self.before.charge.clone();
        }
        prepared.pending = Some(self.before.clone());
        prepared.last_error = None;
        store.save(&self.previous, &prepared)?;
        change.saved = Some(prepared.clone());
        let owned = Owned {
            io,
            store,
            record: &prepared,
        };
        owned.check()?;
        change.dirty = true;
        let result = emit(&owned, &self.policy, &self.before.capability).and_then(|_| {
            if !self.policy.enabled {
                owned.restore_charge(&self.charge)?;
            }
            if owned.charge()? != self.charge {
                bail!("节能关联充电策略回读不一致");
            }
            let mut committed = prepared.clone();
            committed.policy = Some(self.policy.clone());
            committed.pending = None;
            store.save(&Some(prepared.clone()), &committed)?;
            change.saved = Some(committed);
            Ok(())
        });
        if let Err(error) = result {
            return self.fail(io, store, change, prepared, error);
        }
        Ok(())
    }
    fn fail(
        &self,
        io: &dyn Io,
        store: &dyn Store,
        change: &mut Change,
        prepared: Record,
        error: anyhow::Error,
    ) -> Result<()> {
        let recovered = recover(
            &Owned {
                io,
                store,
                record: &prepared,
            },
            &self.before,
        );
        let restored = recovered.is_ok();
        let message = ccc_core::msr::with_recovery(
            format!("节能设置失败：{error:#}"),
            recovered.map_err(|error| format!("{error:#}")),
        );
        let mut failed = self.record_before();
        failed.pending = (!restored).then(|| self.before.clone());
        failed.last_error = Some(message.clone());
        if let Err(error) = store.save(&Some(prepared), &failed) {
            bail!("{message}；恢复记录保存失败：{error:#}");
        }
        change.saved = Some(failed);
        change.dirty = !restored;
        bail!(message)
    }
    pub(in crate::battery) fn rollback(
        &self,
        io: &dyn Io,
        store: &dyn Store,
        change: &mut Change,
    ) -> Result<()> {
        if !change.dirty {
            return Ok(());
        }
        let saved = change.saved.as_ref().context("缺少节能本次写入记录")?;
        recover(
            &Owned {
                io,
                store,
                record: saved,
            },
            &self.before,
        )?;
        let restored = self.record_before();
        store.save(&change.saved, &restored)?;
        change.saved = Some(restored);
        change.dirty = false;
        Ok(())
    }
}

struct Owned<'a> {
    io: &'a dyn Io,
    store: &'a dyn Store,
    record: &'a Record,
}
impl Owned<'_> {
    fn check(&self) -> Result<()> {
        if self.store.load()?.as_ref() != Some(self.record) {
            bail!("节能恢复记录已变化，旧操作不再拥有写入权");
        }
        if self.io.capability()?.as_ref() != Some(&self.record.capability) {
            bail!("节能能力已变化，旧操作不再拥有写入权");
        }
        self.io
            .charge()?
            .same_capabilities(&self.record.baseline.charge)?;
        Ok(())
    }
}
impl Io for Owned<'_> {
    fn capability(&self) -> Result<Option<Capability>> {
        self.io.capability()
    }
    fn charge(&self) -> Result<ChargeState> {
        self.io.charge()
    }
    fn present(&self) -> Result<bool> {
        self.io.present()
    }
    fn original(&self, cap: &Capability) -> Result<Option<Policy>> {
        self.io.original(cap)
    }
    fn clock(&self) -> Clock {
        self.io.clock()
    }
    fn send(&self, command: u32) -> Result<()> {
        self.check()?;
        self.io.send(command)
    }
    fn restore_charge(&self, charge: &ChargeState) -> Result<()> {
        self.check()?;
        self.io.restore_charge(charge)
    }
}

pub(super) fn restore(
    io: &dyn Io,
    store: &dyn Store,
    before: Snapshot,
    record: Record,
    original: Snapshot,
) -> Result<()> {
    let mut prepared = record.clone();
    prepared.pending = Some(before);
    prepared.last_error = None;
    store.save(&Some(record), &prepared)?;
    let mut saved = prepared.clone();
    if let Err(error) = recover(
        &Owned {
            io,
            store,
            record: &prepared,
        },
        &original,
    ) {
        saved.last_error = Some(format!("节能初始恢复失败：{error:#}"));
        let saved_error = store.save(&Some(prepared), &saved).err();
        bail!(
            "节能初始恢复失败：{error:#}{}",
            saved_error.map_or(String::new(), |error| format!("；记录保存：{error:#}"))
        );
    }
    saved.policy = original.policy;
    saved.stop_charge = original.charge;
    saved.pending = None;
    store.save(&Some(prepared), &saved)
}
