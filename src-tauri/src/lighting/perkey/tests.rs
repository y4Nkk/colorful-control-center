use super::*;
use std::{cell::RefCell, collections::BTreeMap};
struct Fake {
    regions: RefCell<BTreeMap<(i32, i32), Vec<u8>>>,
    commands: RefCell<Vec<[u8; 5]>>,
    writes: RefCell<usize>,
    fail: RefCell<Vec<usize>>,
    identity: RefCell<String>,
}
impl Fake {
    fn new() -> Self {
        let mut settings = vec![0u8; 76];
        settings[3] = 4;
        settings[48] = 1;
        settings[2] = 2;
        Self {
            regions: RefCell::new(BTreeMap::from([
                ((2, 32), settings),
                ((2, 128), vec![17; 128]),
                ((3, 0), vec![17; 256]),
            ])),
            commands: RefCell::new(vec![]),
            writes: RefCell::new(0),
            fail: RefCell::new(vec![]),
            identity: RefCell::new("hid-A".into()),
        }
    }
    fn request(&self) -> Request {
        Request {
            expected_revision: revision(self, &read(self).unwrap()),
            values: Values {
                brightness: 3,
                enabled: true,
                speed: 2,
                pattern: Pattern::Static {
                    colors: vec![Rgb { r: 7, g: 8, b: 9 }; 120],
                },
            },
        }
    }
}
impl Io for Fake {
    fn device(&self) -> device::DeviceInfo {
        device::decode([0, 0, 0], [1, 0, 0])
    }
    fn identity(&self) -> String {
        self.identity.borrow().clone()
    }
    fn read(&self, p: i32, o: i32, n: usize) -> Result<Vec<u8>> {
        let regions = self.regions.borrow();
        let ((_, start), bytes) = regions
            .iter()
            .find(|((page, start), b)| {
                *page == p && o >= *start && (o - *start) as usize + n <= b.len()
            })
            .expect("有效范围");
        Ok(bytes[(o - *start) as usize..(o - *start) as usize + n].to_vec())
    }
    fn write(&self, p: i32, o: i32, b: &[u8]) -> Result<()> {
        let mut n = self.writes.borrow_mut();
        *n += 1;
        if self.fail.borrow().contains(&*n) {
            anyhow::bail!("region failure");
        }
        let mut regions = self.regions.borrow_mut();
        let ((_, start), bytes) = regions
            .iter_mut()
            .find(|((page, start), data)| {
                *page == p && o >= *start && (o - *start) as usize + b.len() <= data.len()
            })
            .expect("有效范围");
        bytes[(o - *start) as usize..(o - *start) as usize + b.len()].copy_from_slice(b);
        Ok(())
    }
    fn send(&self, c: [u8; 5]) -> Result<()> {
        self.commands.borrow_mut().push(c);
        Ok(())
    }
}
#[test]
fn feature_report_and_matrix_padding_are_exact() {
    assert_eq!(
        &hid::report([1, 0xA3, 7, 8, 9])[..7],
        &[0xCC, 1, 0xA3, 7, 8, 9, 0]
    );
    let io = Fake::new();
    let before = read(&io).unwrap();
    let mut baseline = None;
    apply(&io, &mut baseline, 3, &io.request()).unwrap();
    let after = read(&io).unwrap();
    for i in 0..384 {
        if !(0..120).any(|key| (matrix_offset(key)..matrix_offset(key) + 3).contains(&i)) {
            assert_eq!(before.matrix[i], after.matrix[i]);
        }
    }
    assert_eq!(io.commands.borrow().len(), 122);
    assert_eq!(io.commands.borrow()[121], [9, 6, 2, 0, 0]);
    assert_eq!(after.at(33), 1);
}
#[test]
fn stale_preview_and_invalid_contract_never_write() {
    let io = Fake::new();
    let mut request = io.request();
    request.expected_revision = "stale".into();
    assert!(apply(&io, &mut None, 3, &request).is_err());
    request = io.request();
    if let Pattern::Static { colors } = &mut request.values.pattern {
        colors.pop();
    }
    assert!(apply(&io, &mut None, 3, &request).is_err());
    assert!(io.commands.borrow().is_empty());
    assert_eq!(*io.writes.borrow(), 0);
}
#[test]
fn partial_cross_page_failure_restores_dynamic_mode_and_all_regions() {
    let io = Fake::new();
    io.regions.borrow_mut().get_mut(&(2, 32)).unwrap()[0] = 2;
    let original = io.regions.borrow().clone();
    *io.fail.borrow_mut() = vec![6];
    let mut baseline = None;
    assert!(apply(&io, &mut baseline, 3, &io.request()).is_err());
    assert_eq!(*io.regions.borrow(), original);
    assert!(io.commands.borrow().contains(&[11, 0, 0, 0, 0]));
    assert!(baseline.is_some());
}
#[test]
fn restore_attempts_each_saved_region_despite_failures_and_can_retry() {
    let io = Fake::new();
    let original = read(&io).unwrap();
    *io.fail.borrow_mut() = vec![1, 2];
    assert!(restore(&io, &original).is_err());
    assert_eq!(*io.writes.borrow(), 6);
    restore(&io, &original).unwrap();
}
#[test]
fn device_identity_changes_reject_even_identical_saved_values() {
    let io = Fake::new();
    let request = io.request();
    *io.identity.borrow_mut() = "hid-B".into();
    assert!(apply(&io, &mut None, 3, &request).is_err());
    assert!(io.commands.borrow().is_empty());
}
#[test]
fn disabled_snapshot_never_relights_and_unknown_modes_reject() {
    let io = Fake::new();
    io.regions.borrow_mut().get_mut(&(2, 32)).unwrap()[48] = 0;
    emit(&io, &read(&io).unwrap()).unwrap();
    assert_eq!(io.commands.borrow().last(), Some(&[9, 0, 0, 0, 0]));
    io.regions.borrow_mut().get_mut(&(2, 32)).unwrap()[0] = 255;
    assert!(read(&io).is_err());
}
#[test]
fn dynamic_direction_and_colors_restore_original_packets() {
    let io = Fake::new();
    {
        let mut r = io.regions.borrow_mut();
        let s = r.get_mut(&(2, 32)).unwrap();
        s[0] = 7;
        s[32] = 10;
        s[34..37].copy_from_slice(&[1, 2, 3]);
    }
    let s = read(&io).unwrap();
    assert_eq!(effects::dynamic(&s).unwrap(), vec![[21, 163, 1, 2, 3]]);
    for mode in 0..=11 {
        let mut s = s.clone();
        s.settings[0] = mode;
        s.settings[28] = 0;
        assert!(!effects::dynamic(&s).unwrap().is_empty());
    }
}

#[test]
fn complete_patterns_round_trip_and_reject_unsupported_targets_before_output() {
    let io = Fake::new();
    let before = read(&io).unwrap();
    let color = Some(Rgb {
        r: 10,
        g: 20,
        b: 30,
    });
    let patterns = vec![
        Pattern::Wave {
            direction: 7,
            color,
        },
        Pattern::Breath { color },
        Pattern::Blink { color },
        Pattern::Random { color },
        Pattern::Scan {
            colors: Some([color.unwrap(); 2]),
        },
        Pattern::Snake {
            direction: 3,
            color,
        },
        Pattern::Ripple {
            reactive: true,
            color,
        },
        Pattern::Fn {
            color: color.unwrap(),
        },
        Pattern::Fade { color },
    ];
    for pattern in patterns {
        let values = Values {
            brightness: 4,
            speed: 2,
            enabled: true,
            pattern,
        };
        let target = model::target(&before, &values, &io.device()).unwrap();
        assert_eq!(effects::values(&target).unwrap(), values);
        assert!(!effects::dynamic(&target).unwrap().is_empty());
    }
    let mut legacy = io.device();
    legacy.new_effects = false;
    for pattern in [
        Pattern::Wave {
            direction: 1,
            color: None,
        },
        Pattern::Random { color },
        Pattern::Snake {
            direction: 0,
            color,
        },
    ] {
        assert!(
            model::target(
                &before,
                &Values {
                    brightness: 3,
                    speed: 1,
                    enabled: true,
                    pattern
                },
                &legacy
            )
            .is_err()
        );
    }
    let mut values = io.request().values;
    values.speed = 3;
    assert!(model::target(&before, &values, &io.device()).is_err());
    assert!(
        serde_json::from_str::<Request>(r#"{"expectedRevision":"old","brightness":3,"colors":[]}"#)
            .is_err()
    );
    assert!(io.commands.borrow().is_empty());
}
#[test]
fn restore_does_not_overwrite_newer_sleep_or_auxiliary_light_settings() {
    let io = Fake::new();
    let mut baseline = None;
    apply(&io, &mut baseline, 3, &io.request()).unwrap();
    {
        let mut regions = io.regions.borrow_mut();
        let settings = regions.get_mut(&(2, 32)).unwrap();
        settings[4..9].fill(0xA5);
        settings[49..76].fill(0x5A);
    }
    restore(&io, &baseline.unwrap().1).unwrap();
    let restored = read(&io).unwrap();
    assert_eq!(&restored.settings[4..9], &[0xA5; 5]);
    assert_eq!(&restored.settings[49..76], &[0x5A; 27]);
}

#[test]
fn scan_allows_one_black_endpoint_but_does_not_silently_replace_all_black() {
    let io = Fake::new();
    let before = read(&io).unwrap();
    let black = Rgb { r: 0, g: 0, b: 0 };
    let red = Rgb { r: 255, g: 0, b: 0 };
    let mut values = Values {
        brightness: 3,
        speed: 1,
        enabled: true,
        pattern: Pattern::Scan {
            colors: Some([black, red]),
        },
    };
    let target = model::target(&before, &values, &io.device()).unwrap();
    assert_eq!(
        effects::dynamic(&target).unwrap(),
        vec![[23, 161, 0, 0, 0], [23, 162, 255, 0, 0]]
    );
    values.pattern = Pattern::Scan {
        colors: Some([black, black]),
    };
    assert!(model::target(&before, &values, &io.device()).is_err());
    values.pattern = Pattern::Breath { color: Some(black) };
    assert!(model::target(&before, &values, &io.device()).is_err());
}

#[test]
fn static_snapshot_does_not_interpret_inactive_dynamic_mode_or_flags() {
    let io = Fake::new();
    {
        let mut regions = io.regions.borrow_mut();
        let settings = regions.get_mut(&(2, 32)).unwrap();
        settings[0] = 255;
        settings[1] = 1;
        settings[16] = 255;
    }
    let snapshot = read(&io).unwrap();
    assert!(matches!(
        effects::values(&snapshot).unwrap().pattern,
        Pattern::Static { .. }
    ));
    emit(&io, &snapshot).unwrap();
    assert_eq!(io.commands.borrow()[0], [0, 12, 0, 0, 0]);
    assert_eq!(io.commands.borrow().len(), 122);
}
