use super::{Device, Result, Step, validate_code};
use std::time::Duration;

#[derive(Default)]
pub struct Recorder {
    steps: Vec<Step>,
    held: Vec<u16>,
    last: Option<Duration>,
    done: bool,
}
#[derive(Debug, PartialEq)]
pub enum Outcome {
    Added,
    Ignored,
    LimitReached,
}
impl Recorder {
    pub fn steps(&self) -> &[Step] {
        &self.steps
    }
    pub fn done(&self) -> bool {
        self.done
    }
    pub fn capture(&mut self, scan: u16, down: bool, now: Duration) -> Result<Outcome> {
        if self.done {
            return Ok(Outcome::Ignored);
        }
        validate_code(Device::Keyboard, scan)?;
        if down && self.held.contains(&scan) || !down && !self.held.contains(&scan) {
            return Ok(Outcome::Ignored);
        }
        let held_after = if down {
            self.held.len() + 1
        } else {
            self.held.len() - 1
        };
        if self.steps.len() + 1 + held_after > 256 {
            self.finish(now)?;
            return Ok(Outcome::LimitReached);
        }
        self.delay(now)?;
        if down {
            self.held.push(scan);
        } else {
            self.held.retain(|code| *code != scan);
        }
        self.steps.push(Step::Key {
            scan_code: scan,
            pressed: down,
            delay_after_ms: 0,
        });
        self.last = Some(now);
        Ok(Outcome::Added)
    }
    fn delay(&mut self, now: Duration) -> Result<()> {
        let delay = match self.last {
            Some(before) => {
                u32::try_from(now.checked_sub(before).ok_or("录制时刻倒退")?.as_millis())
                    .map_err(|_| "录制间隔超过原厂延时范围")?
            }
            None => 0,
        };
        if let Some(Step::Key { delay_after_ms, .. }) = self.steps.last_mut() {
            *delay_after_ms = delay;
        }
        Ok(())
    }
    pub fn finish(&mut self, now: Duration) -> Result<()> {
        if self.done {
            return Ok(());
        }
        self.delay(now)?;
        for code in self.held.drain(..).rev() {
            self.steps.push(Step::Key {
                scan_code: code,
                pressed: false,
                delay_after_ms: 0,
            });
        }
        self.done = true;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn monotonic_delays_autorepeat_and_missing_release_are_handled_without_injecting_keys() {
        let mut recorder = Recorder::default();
        assert_eq!(
            recorder.capture(29, true, Duration::ZERO).unwrap(),
            Outcome::Added
        );
        assert_eq!(
            recorder
                .capture(29, true, Duration::from_millis(50))
                .unwrap(),
            Outcome::Ignored
        );
        recorder
            .capture(30, true, Duration::from_millis(100))
            .unwrap();
        recorder.finish(Duration::from_millis(250)).unwrap();
        assert_eq!(
            recorder.steps(),
            &[
                Step::Key {
                    scan_code: 29,
                    pressed: true,
                    delay_after_ms: 100
                },
                Step::Key {
                    scan_code: 30,
                    pressed: true,
                    delay_after_ms: 150
                },
                Step::Key {
                    scan_code: 30,
                    pressed: false,
                    delay_after_ms: 0
                },
                Step::Key {
                    scan_code: 29,
                    pressed: false,
                    delay_after_ms: 0
                }
            ]
        );
        super::super::validate_steps(recorder.steps()).unwrap();
    }
    #[test]
    fn capacity_reserves_space_for_releases_and_explicitly_reports_stop() {
        let mut recorder = Recorder::default();
        for i in 0..128 {
            recorder
                .capture(30, true, Duration::from_millis(i * 2))
                .unwrap();
            recorder
                .capture(30, false, Duration::from_millis(i * 2 + 1))
                .unwrap();
        }
        assert_eq!(
            recorder
                .capture(31, true, Duration::from_millis(300))
                .unwrap(),
            Outcome::LimitReached
        );
        assert_eq!(recorder.steps().len(), 256);
        assert!(recorder.done());
        super::super::validate_steps(recorder.steps()).unwrap();
    }
    #[test]
    fn timestamp_errors_do_not_append_and_finished_sessions_ignore_later_input() {
        let mut recorder = Recorder::default();
        recorder
            .capture(30, true, Duration::from_millis(100))
            .unwrap();
        assert!(
            recorder
                .capture(30, false, Duration::from_millis(99))
                .is_err()
        );
        assert_eq!(recorder.steps().len(), 1);
        recorder.finish(Duration::from_millis(150)).unwrap();
        assert_eq!(
            recorder
                .capture(31, true, Duration::from_millis(200))
                .unwrap(),
            Outcome::Ignored
        );
    }
}
