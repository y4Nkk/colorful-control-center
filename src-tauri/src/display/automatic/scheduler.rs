use crate::hal::power_events::{Event, Source};
use anyhow::{Result, anyhow};
use ccc_core::oem_refresh::Trigger;
use std::collections::VecDeque;

const STABLE_MS: u64 = 1_000;
mod completed;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Work {
    pub epoch: u64,
    pub trigger: Trigger,
    pub previous_mode: Option<u8>,
    pub ready_at: u64,
    pub completion: Option<u64>,
}
#[derive(Default)]
pub(super) struct Scheduler {
    pub online: bool,
    pub enabled: bool,
    pub sleeping: bool,
    pub stopping: bool,
    pub source: Option<Source>,
    pub mode: Option<u8>,
    pending: VecDeque<Work>,
    notification: Option<Source>,
    epoch: u64,
    completed: Option<completed::Handoff>,
}
impl Scheduler {
    fn advance(&mut self, preserve_at: Option<u64>) -> Result<()> {
        self.epoch = self
            .epoch
            .checked_add(1)
            .ok_or_else(|| anyhow!("自动刷新率事件代次耗尽"))?;
        if let Some(now) = preserve_at {
            for work in &mut self.pending {
                work.epoch = self.epoch;
                work.ready_at = now.saturating_add(STABLE_MS);
            }
            if let Some(completed) = &mut self.completed {
                completed.retry(now);
            }
        } else {
            self.pending.clear();
            if let Some(completed) = &mut self.completed {
                completed.in_flight = false;
            }
        }
        Ok(())
    }
    fn queue(&mut self, trigger: Trigger, previous_mode: Option<u8>, now: u64) {
        if self.enabled && self.online && !self.sleeping && !self.stopping {
            self.pending.push_back(Work {
                epoch: self.epoch,
                trigger,
                previous_mode,
                ready_at: now.saturating_add(STABLE_MS),
                completion: None,
            });
        }
    }
    pub fn ready(&mut self) -> Result<()> {
        self.advance(None)?;
        self.online = true;
        Ok(())
    }
    pub fn configure(&mut self, mode: u8, on_ac: bool, trigger: Trigger, now: u64) -> Result<()> {
        if !self.online || self.stopping || self.sleeping {
            return Err(anyhow!("电源通知未就绪或系统正在休眠"));
        }
        let source = if on_ac { Source::Ac } else { Source::Battery };
        if !self.supply_matches(source) {
            return Err(anyhow!("供电通知与系统状态不一致"));
        }
        self.advance(None)?;
        self.enabled = true;
        self.mode = Some(mode);
        self.source = Some(source);
        self.retain_completed(mode);
        let previous_mode = (trigger == Trigger::Startup).then_some(mode);
        self.queue(trigger, previous_mode, now);
        Ok(())
    }
    pub fn disable(&mut self) -> Result<()> {
        self.advance(None)?;
        self.completed = None;
        self.enabled = false;
        Ok(())
    }
    pub fn fail(&mut self) -> Result<()> {
        self.advance(None)?;
        self.completed = None;
        self.online = false;
        Ok(())
    }
    pub fn stop(&mut self) -> Result<()> {
        self.advance(None)?;
        self.completed = None;
        self.stopping = true;
        self.online = false;
        Ok(())
    }
    pub fn observe(&mut self, event: Event, now: u64) -> Result<()> {
        match event {
            Event::Source(source) => {
                self.notification = Some(source);
                self.supply(source, now)
            }
            Event::Suspend if !self.sleeping => {
                self.advance(None)?;
                self.completed = None;
                self.sleeping = true;
                Ok(())
            }
            Event::Resume if self.sleeping => {
                self.advance(None)?;
                self.sleeping = false;
                self.mode = None;
                self.source = None;
                self.notification = None;
                Ok(())
            }
            _ => Ok(()),
        }
    }
    pub fn supply(&mut self, source: Source, now: u64) -> Result<()> {
        if self.source == Some(source) {
            return Ok(());
        }
        let initial = self.source.is_none();
        let known = matches!(source, Source::Ac | Source::Battery);
        self.advance(known.then_some(now))?;
        self.source = Some(source);
        self.pending
            .retain(|work| work.trigger != Trigger::SupplyChange);
        if !initial && known {
            self.queue(Trigger::SupplyChange, None, now);
        }
        Ok(())
    }
    pub fn read_supply(&mut self, source: Source, now: u64) -> Result<()> {
        let source = match self.notification {
            Some(Source::Ups | Source::Unknown) => Source::Unknown,
            Some(notified) if notified != source => Source::Unknown,
            _ => source,
        };
        self.supply(source, now)
    }
    pub fn mode(&mut self, mode: u8, now: u64) -> Result<()> {
        if self.mode == Some(mode) {
            return Ok(());
        }
        let previous = self
            .pending
            .iter()
            .find(|work| work.trigger == Trigger::ModeChange)
            .and_then(|work| work.previous_mode)
            .or(self.mode);
        self.advance(Some(now))?;
        self.completed = None;
        self.mode = Some(mode);
        self.pending
            .retain(|work| work.trigger != Trigger::ModeChange);
        if previous.is_some() {
            self.queue(Trigger::ModeChange, previous, now);
        }
        Ok(())
    }
    pub fn take(&mut self, now: u64) -> Option<Work> {
        if self.completed.is_some() {
            return self.take_completed(now);
        }
        if self
            .pending
            .front()
            .is_some_and(|work| work.ready_at <= now && self.permits(work.epoch))
        {
            return self.pending.pop_front();
        }
        None
    }
    pub fn permits(&self, epoch: u64) -> bool {
        self.enabled
            && self.online
            && !self.sleeping
            && !self.stopping
            && self.epoch == epoch
            && matches!(self.source, Some(Source::Ac | Source::Battery))
    }
    pub fn supply_known(&self) -> bool {
        matches!(self.source, Some(Source::Ac | Source::Battery))
    }
    pub fn supply_matches(&self, source: Source) -> bool {
        matches!(source, Source::Ac | Source::Battery)
            && self.notification.is_none_or(|notified| notified == source)
    }
    pub fn ticket(&self) -> u64 {
        self.epoch
    }
    pub fn permits_setup(&self, epoch: u64) -> bool {
        self.online && !self.sleeping && !self.stopping && self.epoch == epoch
    }
}
