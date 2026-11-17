use super::model::Trigger;
use crate::hal::power_events::{Event, Source};
use anyhow::{Result, anyhow};

pub(super) const STABLE_MS: u64 = 1_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Pending {
    pub trigger: Trigger,
    pub epoch: u64,
    pub ready_at: u64,
}

#[derive(Default)]
pub(super) struct Scheduler {
    pub source: Option<Source>,
    pub epoch: u64,
    pub enabled: bool,
    pub rules: [bool; 3],
    pub pending: Option<Pending>,
    system_sleeping: bool,
    display_sleeping: bool,
    display_initialized: bool,
}

impl Scheduler {
    pub fn sleeping(&self) -> bool {
        self.system_sleeping || self.display_sleeping
    }

    fn invalidate(&mut self) -> Result<()> {
        let Some(epoch) = self.epoch.checked_add(1) else {
            self.enabled = false;
            self.pending = None;
            return Err(anyhow!("自动档案事件代次已耗尽"));
        };
        self.epoch = epoch;
        Ok(())
    }

    pub fn configure(&mut self, enabled: bool, rules: [bool; 3]) -> Result<()> {
        self.invalidate()?;
        self.pending = None;
        self.enabled = enabled;
        self.rules = rules;
        Ok(())
    }

    fn queue(&mut self, trigger: Option<Trigger>, now: u64) {
        self.pending = trigger
            .filter(|trigger| self.enabled && self.rules[trigger.index()])
            .map(|trigger| Pending {
                trigger,
                epoch: self.epoch,
                ready_at: now.saturating_add(STABLE_MS),
            });
    }

    pub fn observe(&mut self, event: Event, now: u64) -> Result<()> {
        match event {
            Event::Source(source) => self.supply(source, now)?,
            Event::Suspend | Event::Resume => {
                let sleeping = matches!(event, Event::Suspend);
                if self.system_sleeping == sleeping {
                    return Ok(());
                }
                let before = self.sleeping();
                self.invalidate()?;
                self.system_sleeping = sleeping;
                self.sleep_transition(before, now);
            }
            Event::DisplayAwake(awake) => {
                let first = !self.display_initialized;
                self.display_initialized = true;
                if self.display_sleeping == !awake {
                    return Ok(());
                }
                let before = self.sleeping();
                self.invalidate()?;
                self.display_sleeping = !awake;
                if first {
                    self.pending = None;
                } else {
                    self.sleep_transition(before, now);
                }
            }
        }
        Ok(())
    }

    fn supply(&mut self, source: Source, now: u64) -> Result<()> {
        if self.source == Some(source) {
            return Ok(());
        }
        let first = self.source.is_none();
        self.invalidate()?;
        self.source = Some(source);
        if first {
            let resume = self
                .pending
                .filter(|p| p.trigger == Trigger::Resume)
                .map(|p| p.trigger);
            self.queue(resume, now);
            return Ok(());
        }
        let trigger = if self.pending.is_some_and(|p| p.trigger == Trigger::Resume) {
            Some(Trigger::Resume)
        } else {
            match source {
                Source::Ac => Some(Trigger::Ac),
                Source::Battery => Some(Trigger::Battery),
                _ => None,
            }
        };
        self.queue(trigger, now);
        Ok(())
    }

    fn sleep_transition(&mut self, before: bool, now: u64) {
        if !before && self.sleeping() {
            self.pending = None;
        } else if before && !self.sleeping() {
            let trigger = if self.rules[Trigger::Resume.index()] {
                Some(Trigger::Resume)
            } else {
                self.pending.map(|p| p.trigger)
            };
            self.queue(trigger, now);
        } else if let Some(pending) = &mut self.pending {
            pending.epoch = self.epoch;
        }
    }

    pub fn take(&mut self, now: u64) -> Option<Pending> {
        if !self.enabled || self.sleeping() || self.pending.is_none_or(|p| p.ready_at > now) {
            return None;
        }
        self.pending.take()
    }

    pub fn permits(&self, epoch: u64) -> bool {
        self.enabled && !self.sleeping() && self.epoch == epoch
    }
}
