use crate::hal::power_events::{Event, Source};
use anyhow::{Result, anyhow, bail};
use ccc_core::oem_supply_mode::Trigger;

const STABLE_MS: u64 = 1_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Work {
    pub epoch: u64,
    pub source: Source,
    pub trigger: Trigger,
    pub ready_at: u64,
}

#[derive(Default)]
pub(super) struct Scheduler {
    pub online: bool,
    pub enabled: bool,
    pub sleeping: bool,
    pub stopping: bool,
    notification: Option<Source>,
    source: Option<Source>,
    last_known: Option<Source>,
    pending: Option<Work>,
    epoch: u64,
}

impl Scheduler {
    fn invalidate(&mut self, now: u64, preserve: bool) -> Result<()> {
        self.epoch = self
            .epoch
            .checked_add(1)
            .ok_or_else(|| anyhow!("供电模式事件代次耗尽"))?;
        if preserve {
            if let Some(work) = &mut self.pending {
                work.epoch = self.epoch;
                work.ready_at = now.saturating_add(STABLE_MS);
            }
        } else {
            self.pending = None;
        }
        Ok(())
    }
    fn queue(&mut self, trigger: Trigger, now: u64) {
        if let Some(source @ (Source::Ac | Source::Battery)) = self.source {
            if self.online && self.enabled && !self.sleeping && !self.stopping {
                self.pending = Some(Work {
                    epoch: self.epoch,
                    source,
                    trigger,
                    ready_at: now.saturating_add(STABLE_MS),
                });
            }
        }
    }
    pub fn ready(&mut self) -> Result<()> {
        self.invalidate(0, false)?;
        self.online = true;
        Ok(())
    }
    pub fn configure(&mut self, source: Source, trigger: Trigger, now: u64) -> Result<()> {
        if !self.online || self.sleeping || self.stopping || !self.matches(source) {
            bail!("电源通知未就绪、供电状态不一致或系统正在休眠");
        }
        self.invalidate(now, false)?;
        self.enabled = true;
        self.source = Some(source);
        self.last_known = Some(source);
        self.queue(trigger, now);
        Ok(())
    }
    pub fn disable(&mut self) -> Result<()> {
        self.invalidate(0, false)?;
        self.enabled = false;
        Ok(())
    }
    pub fn fail(&mut self) -> Result<()> {
        self.invalidate(0, false)?;
        self.online = false;
        Ok(())
    }
    pub fn stop(&mut self) -> Result<()> {
        self.invalidate(0, false)?;
        self.stopping = true;
        self.online = false;
        Ok(())
    }
    pub fn observe(&mut self, event: Event, now: u64) -> Result<()> {
        match event {
            Event::Source(source) if self.notification != Some(source) => {
                self.notification = Some(source);
                self.invalidate(now, true)?;
                self.source = None;
            }
            Event::Suspend if !self.sleeping => {
                self.invalidate(now, false)?;
                self.sleeping = true;
                self.source = None;
            }
            Event::Resume if self.sleeping => {
                self.invalidate(now, false)?;
                self.sleeping = false;
                self.notification = None;
                self.source = None;
                if self.enabled && self.online && !self.stopping {
                    self.pending = Some(Work {
                        epoch: self.epoch,
                        source: Source::Unknown,
                        trigger: Trigger::Resume,
                        ready_at: now.saturating_add(STABLE_MS),
                    });
                }
            }
            _ => (),
        }
        Ok(())
    }
    pub fn read_source(&mut self, source: Source, now: u64) -> Result<()> {
        if !self.matches(source) {
            if self.source.is_some() {
                self.invalidate(now, true)?;
                self.source = None;
            }
            return Ok(());
        }
        if self.source == Some(source) {
            return Ok(());
        }
        self.invalidate(now, true)?;
        self.source = Some(source);
        let changed = self.last_known.is_some_and(|last| last != source);
        self.last_known = Some(source);
        if let Some(work) = &mut self.pending {
            work.source = source;
            if changed && work.trigger != Trigger::Resume {
                work.trigger = Trigger::SupplyChange;
            }
        } else if changed {
            self.queue(Trigger::SupplyChange, now);
        }
        Ok(())
    }
    pub fn take(&mut self, now: u64) -> Option<Work> {
        let work = self.pending?;
        if work.ready_at > now || !self.permits(work.epoch, work.source) {
            return None;
        }
        self.pending.take()
    }
    pub fn permits(&self, epoch: u64, source: Source) -> bool {
        self.enabled
            && self.online
            && !self.sleeping
            && !self.stopping
            && self.epoch == epoch
            && self.source == Some(source)
            && self.matches(source)
    }
    pub fn matches(&self, source: Source) -> bool {
        matches!(source, Source::Ac | Source::Battery)
            && self.notification.is_none_or(|notified| notified == source)
    }
    pub fn ticket(&self) -> u64 {
        self.epoch
    }
    pub fn has_pending(&self) -> bool {
        self.pending.is_some()
    }
    pub fn permits_setup(&self, epoch: u64) -> bool {
        self.online && !self.sleeping && !self.stopping && self.epoch == epoch
    }
}

#[cfg(test)]
mod tests;
