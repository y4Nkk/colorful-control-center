use super::{Pattern, device::DeviceInfo};
use std::{collections::BTreeSet, time::Duration};

#[derive(Clone, Copy, Debug)]
pub struct KeyEvent {
    pub scan: u16,
    pub down: bool,
}
#[derive(Default)]
pub struct Engine {
    held: BTreeSet<u16>,
    win_due: Option<Duration>,
    chord: [bool; 3],
}
impl Engine {
    pub fn event(
        &mut self,
        event: KeyEvent,
        now: Duration,
        pattern: &Pattern,
        device: &DeviceInfo,
    ) -> Vec<[u8; 5]> {
        let mut commands = self.tick(now, pattern);
        let scan = if event.scan == 0xE036 {
            0x36
        } else {
            event.scan
        };
        if !event.down {
            self.held.remove(&scan);
            self.chord = [false; 3];
            if matches!(pattern, Pattern::Fn { .. }) && scan == 0x6F {
                commands.push([0, 12, 0, 0, 0]);
            }
            return commands;
        }
        if scan == 0 || !self.held.insert(scan) {
            return commands;
        }
        if let Pattern::Fn { color } = pattern {
            if scan == 0x6F {
                commands.extend(
                    fn_leds(device)
                        .into_iter()
                        .map(|id| [1, id, color.r, color.g, color.b]),
                );
            }
            return commands;
        }
        if scan == 0xE05B {
            self.chord[0] = true;
            self.win_due = Some(now + Duration::from_millis(50));
            return commands;
        }
        if scan == 0x2A {
            self.win_due = None;
            self.chord[1] = true;
            if self.chord[0] {
                return commands;
            }
        }
        if scan == 0x6E {
            self.chord[2] = true;
        }
        let key = if self.chord.iter().all(|v| *v) {
            self.chord = [false; 3];
            0xDD
        } else if scan >> 8 == 0xE0 {
            scan as u8 | 0x80
        } else {
            scan as u8
        };
        commands.extend(trigger(pattern, key));
        commands
    }
    pub fn tick(&mut self, now: Duration, pattern: &Pattern) -> Vec<[u8; 5]> {
        if self.win_due.is_some_and(|deadline| now >= deadline) {
            self.win_due = None;
            return trigger(pattern, 0xDB).into_iter().collect();
        }
        vec![]
    }
}
fn trigger(pattern: &Pattern, key: u8) -> Option<[u8; 5]> {
    let (command, color) = match pattern {
        Pattern::Ripple { reactive, color } => (if *reactive { 8 } else { 7 }, color),
        Pattern::Fade { color } => (8, color),
        _ => return None,
    };
    Some(match color {
        Some(c) => [command, key, c.r, c.g, c.b],
        None => [command, 0xFF, key, 0, 0],
    })
}
fn fn_leds(device: &DeviceInfo) -> Vec<u8> {
    let mut ids = vec![0, 1, 2, 3, 5, 6, 7, 8, 9, 10, 11, 12, 33, 35, 51, 83];
    if device.three_column {
        ids.extend([48, 49, 47]);
    } else {
        ids.extend([49, 50, 36]);
        if device.language != 3 {
            ids.push(115);
        }
        ids.push(47);
        if device.language != 2 {
            ids.push(46);
        }
    }
    ids.push(162);
    ids
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lighting::protocol::Rgb;
    fn key(scan: u16, down: bool) -> KeyEvent {
        KeyEvent { scan, down }
    }
    #[test]
    fn original_extended_codes_repeat_filter_and_reactive_packets() {
        let device = super::super::device::decode([0; 3], [1, 0, 0]);
        let pattern = Pattern::Ripple {
            reactive: false,
            color: None,
        };
        let mut e = Engine::default();
        assert_eq!(
            e.event(key(0xE01D, true), Duration::ZERO, &pattern, &device),
            vec![[7, 255, 0x9D, 0, 0]]
        );
        assert!(
            e.event(key(0xE01D, true), Duration::ZERO, &pattern, &device)
                .is_empty()
        );
        e.event(key(0xE01D, false), Duration::ZERO, &pattern, &device);
        assert_eq!(
            e.event(
                key(0xE036, true),
                Duration::ZERO,
                &Pattern::Fade {
                    color: Some(Rgb { r: 1, g: 2, b: 3 })
                },
                &device
            ),
            vec![[8, 0x36, 1, 2, 3]]
        );
    }
    #[test]
    fn delayed_win_and_copilot_chord_are_distinct() {
        let device = super::super::device::decode([0; 3], [1, 0, 0]);
        let p = Pattern::Ripple {
            reactive: true,
            color: None,
        };
        let mut e = Engine::default();
        assert!(
            e.event(key(0xE05B, true), Duration::ZERO, &p, &device)
                .is_empty()
        );
        assert!(e.tick(Duration::from_millis(49), &p).is_empty());
        assert_eq!(
            e.tick(Duration::from_millis(50), &p),
            vec![[8, 255, 0xDB, 0, 0]]
        );
        let mut e = Engine::default();
        e.event(key(0xE05B, true), Duration::ZERO, &p, &device);
        assert!(
            e.event(key(0x2A, true), Duration::ZERO, &p, &device)
                .is_empty()
        );
        assert_eq!(
            e.event(key(0x6E, true), Duration::ZERO, &p, &device),
            vec![[8, 255, 0xDD, 0, 0]]
        );
        assert!(e.tick(Duration::from_secs(1), &p).is_empty());
    }
    #[test]
    fn fn_uses_model_and_language_specific_leds_and_clears_on_release() {
        let p = Pattern::Fn {
            color: Rgb {
                r: 10,
                g: 20,
                b: 30,
            },
        };
        for (raw, missing) in [
            (0, vec![48]),
            (2, vec![46, 48]),
            (3, vec![115, 48]),
            (32, vec![50, 36, 115, 46]),
        ] {
            let d = super::super::device::decode([raw, 0, 0], [1, 0, 0]);
            let mut e = Engine::default();
            let commands = e.event(key(0x6F, true), Duration::ZERO, &p, &d);
            assert!(commands.contains(&[1, 162, 10, 20, 30]));
            for id in missing {
                assert!(!commands.iter().any(|c| c[1] == id));
            }
            assert_eq!(
                e.event(key(0x6F, false), Duration::ZERO, &p, &d),
                vec![[0, 12, 0, 0, 0]]
            );
        }
    }
}
