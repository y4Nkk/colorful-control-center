use super::*;

/// One persisted choice and its compensating request share the original baseline.
pub(super) struct Plan {
    before: Preview,
    request: Record,
    owned: Option<Loaded>,
    dirty: bool,
    noop: bool,
}
impl Plan {
    pub fn dirty(&self) -> bool {
        self.dirty
    }

    pub fn new(before: Preview, target: u8, restoring: bool) -> Result<Self> {
        if !before.observed.supports(target) {
            bail!("固件未声明支持此显卡模式");
        }
        let uncertain = before.loaded.record.as_ref().is_some_and(|record| {
            record.verification(&before.observed, &before.identity) == Verification::Uncertain
        });
        if uncertain && !restoring {
            bail!("此前显卡选择尚未确认，请先恢复选择记录");
        }
        let noop = !restoring && profile::known(&before) == target;
        let request = pending(&before, target, restoring);
        Ok(Self {
            before,
            request,
            owned: None,
            dirty: false,
            noop,
        })
    }
    pub fn execute(&mut self, io: &(impl Io + ?Sized), store: &impl Store) -> Result<()> {
        let current = preview(io, store)?.context("显卡模式接口已不可用")?;
        check_revision(&current, &self.before.revision)?;
        if self.noop {
            return Ok(());
        }
        if self.owned.is_some() {
            bail!("同一次显卡选择不能重复执行");
        }
        self.owned = Some(store.replace(&self.before.loaded, &self.request)?);
        self.dirty = true;
        send(io, &self.before, self.request.target)?;
        let mut accepted = self.request.clone();
        accepted.phase = Phase::Accepted;
        self.owned = Some(store.replace(self.owned.as_ref().unwrap(), &accepted)?);
        Ok(())
    }
    pub fn rollback(&mut self, io: &(impl Io + ?Sized), store: &impl Store) -> Result<()> {
        if !self.dirty {
            return Ok(());
        }
        let current = preview(io, store)?.context("恢复时显卡模式接口不可用")?;
        if current.identity.boot != self.before.identity.boot
            || current.observed != self.before.observed
            || current.loaded.revision != self.owned.as_ref().unwrap().revision
        {
            bail!("显卡模式、启动或记录已变化，未覆盖外部选择");
        }
        let mut recovery = self.request.clone();
        recovery.target = recovery.previous_mode;
        recovery.phase = Phase::Accepted;
        send(io, &self.before, recovery.target)?;
        self.owned = Some(store.replace(self.owned.as_ref().unwrap(), &recovery)?);
        self.dirty = false;
        Ok(())
    }
}
