use super::{STABLE_MS, Scheduler, Work};
use crate::hal::power_events::Source;
use anyhow::{Result, bail};
use ccc_core::{oem_refresh::Trigger, oem_supply_mode};

pub(super) struct Handoff {
    id: u64,
    mode: u8,
    previous_mode: u8,
    source: Source,
    trigger: Trigger,
    ready_at: u64,
    pub(super) in_flight: bool,
}
impl Handoff {
    pub(super) fn retry(&mut self, now: u64) {
        self.in_flight = false;
        self.ready_at = now.saturating_add(STABLE_MS);
    }
}
impl Scheduler {
    pub(in crate::display::automatic) fn completed(
        &mut self,
        mode: u8,
        previous_mode: u8,
        on_ac: bool,
        trigger: oem_supply_mode::Trigger,
        dc_mode_power: bool,
        now: u64,
    ) -> Result<()> {
        if mode > 4 || previous_mode > 4 {
            bail!("供电模式的刷新率交接包含未知模式");
        }
        if self.stopping {
            bail!("自动刷新率正在退出，未接收模式交接");
        }
        let trigger = match (trigger, dc_mode_power) {
            (oem_supply_mode::Trigger::SupplyChange, _) => Trigger::DcModeChange,
            (oem_supply_mode::Trigger::Startup, true) => Trigger::DcModeChange,
            (oem_supply_mode::Trigger::Resume, true) => Trigger::Resume,
            _ => Trigger::ModeChange,
        };
        self.advance(None)?;
        self.mode = Some(mode);
        self.completed = Some(Handoff {
            id: self.epoch,
            mode,
            previous_mode,
            source: if on_ac { Source::Ac } else { Source::Battery },
            trigger,
            ready_at: now.saturating_add(STABLE_MS),
            in_flight: false,
        });
        Ok(())
    }
    pub(super) fn retain_completed(&mut self, mode: u8) {
        if self
            .completed
            .as_ref()
            .is_some_and(|completed| completed.mode != mode)
        {
            self.completed = None;
        }
    }
    pub(super) fn take_completed(&mut self, now: u64) -> Option<Work> {
        let completed = self.completed.as_ref()?;
        if completed.in_flight
            || completed.ready_at > now
            || !self.permits(self.epoch)
            || self.source != Some(completed.source)
            || !self.supply_matches(completed.source)
            || self.mode != Some(completed.mode)
        {
            return None;
        }
        let work = Work {
            epoch: self.epoch,
            trigger: completed.trigger,
            previous_mode: Some(completed.previous_mode),
            ready_at: completed.ready_at,
            completion: Some(completed.id),
        };
        self.completed.as_mut()?.in_flight = true;
        self.pending
            .retain(|work| work.trigger == Trigger::UserChange);
        Some(work)
    }
    pub(in crate::display::automatic) fn acknowledge(&mut self, work: Work) {
        if work.completion.is_some()
            && self.completed.as_ref().map(|completed| completed.id) == work.completion
        {
            self.completed = None;
        }
    }
}
