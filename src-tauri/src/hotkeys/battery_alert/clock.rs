use anyhow::{Result, ensure};
use std::time::{Duration, Instant};

#[derive(Default)]
pub(super) struct Clock {
    playing: bool,
    pending: Option<Instant>,
    closed: bool,
}
impl Clock {
    pub fn ensure_open(&self) -> Result<()> {
        ensure!(!self.closed, "电池音频会话已取消");
        Ok(())
    }
    // 先登记播放所有权；底层启动部分失败时仍须尝试停止本进程的音频。
    pub fn begin(&mut self) -> Result<bool> {
        ensure!(!self.closed, "电池音频会话已取消");
        if self.playing {
            return Ok(false);
        }
        self.playing = true;
        Ok(true)
    }
    pub fn has_work(&self) -> bool {
        self.playing || self.pending.is_some()
    }
    pub fn started(&mut self, now: Instant) {
        // Start不会重置仍Enabled的定时器；187只停止播放器。
        self.pending.get_or_insert(now + Duration::from_secs(1));
    }
    pub fn needs_stop(&self) -> bool {
        self.playing
    }
    pub fn stopped(&mut self) {
        self.playing = false;
    }
    pub fn take_due(&mut self, now: Instant) -> bool {
        if self.closed || self.pending.is_none_or(|deadline| now < deadline) {
            return false;
        }
        self.pending = None;
        true
    }
    pub fn cancel(&mut self) {
        self.closed = true;
        self.pending = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn later(now: Instant, millis: u64) -> Instant {
        now + Duration::from_millis(millis)
    }
    fn start(clock: &mut Clock, now: Instant) -> Result<bool> {
        let needed = clock.begin()?;
        if needed {
            clock.started(now);
        }
        Ok(needed)
    }
    #[test]
    fn first_start_has_one_monotonic_deadline_and_repeat_does_not_extend_it() {
        let now = Instant::now();
        let mut clock = Clock::default();
        assert!(start(&mut clock, now).unwrap());
        assert!(!start(&mut clock, later(now, 800)).unwrap());
        assert!(!clock.take_due(later(now, 999)));
        assert!(clock.take_due(later(now, 1000)));
        assert!(!clock.take_due(later(now, 2000)));
    }
    #[test]
    fn original_stop_does_not_cancel_the_already_queued_notification() {
        let now = Instant::now();
        let mut clock = Clock::default();
        start(&mut clock, now).unwrap();
        clock.stopped();
        assert!(!clock.needs_stop());
        assert!(clock.take_due(later(now, 1000)));
    }
    #[test]
    fn restart_after_stop_keeps_an_existing_timer_instead_of_postponing_it() {
        let now = Instant::now();
        let mut clock = Clock::default();
        start(&mut clock, now).unwrap();
        clock.stopped();
        assert!(start(&mut clock, later(now, 500)).unwrap());
        assert!(clock.take_due(later(now, 1000)));
        assert!(!clock.take_due(later(now, 1500)));
    }
    #[test]
    fn still_playing_does_not_reschedule_after_the_one_shot_has_run() {
        let now = Instant::now();
        let mut clock = Clock::default();
        start(&mut clock, now).unwrap();
        assert!(clock.take_due(later(now, 1000)));
        assert!(!start(&mut clock, later(now, 1100)).unwrap());
        assert!(!clock.take_due(later(now, 3000)));
        clock.stopped();
        assert!(start(&mut clock, later(now, 3100)).unwrap());
        assert!(!clock.take_due(later(now, 4099)));
        assert!(clock.take_due(later(now, 4100)));
    }
    #[test]
    fn policy_cancel_discards_deadline_but_keeps_failed_playback_cleanup_retryable() {
        let now = Instant::now();
        let mut clock = Clock::default();
        start(&mut clock, now).unwrap();
        clock.cancel();
        assert!(clock.needs_stop());
        assert!(!clock.take_due(later(now, 2000)));
        assert!(start(&mut clock, later(now, 2100)).is_err());
        clock.stopped();
        assert!(!clock.needs_stop());
        assert!(start(&mut clock, later(now, 2200)).is_err());
    }
    #[test]
    fn cancellation_before_first_event_never_creates_pending_work() {
        let now = Instant::now();
        let mut clock = Clock::default();
        clock.cancel();
        assert!(start(&mut clock, now).is_err());
        assert!(!clock.needs_stop());
        assert!(!clock.take_due(later(now, 5000)));
    }
}
