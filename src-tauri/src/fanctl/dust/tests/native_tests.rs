use super::*;
use crate::hal::oem_dchu::Dchu;

const LIVE_CURVE: [u8; 14] = [50, 60, 80, 100, 58, 82, 85, 255, 38, 0, 2, 0, 136, 0];
struct Wire {
    pages: [[u8; 256]; 8],
    running: u8,
    dust_reply_tag: u8,
    wmi96: Option<i32>,
    packets: Vec<Vec<u8>>,
    writes: Vec<(i32, Vec<u8>)>,
    fan_mode_bits: u8,
    offset_raw: u8,
    offset_save_failure: Option<i32>,
    offset_command_failure: Option<u8>,
}
impl Default for Wire {
    fn default() -> Self {
        let mut pages = [[0; 256]; 8];
        pages[7][0] = 1;
        pages[7][10] = 128;
        Self {
            pages,
            running: 0,
            dust_reply_tag: 0xB8,
            wmi96: None,
            packets: vec![],
            writes: vec![],
            fan_mode_bits: 0x02,
            offset_raw: 0,
            offset_save_failure: None,
            offset_command_failure: None,
        }
    }
}
thread_local! { static WIRE: RefCell<Wire> = RefCell::new(Wire::default()); }
fn wire<T>(f: impl FnOnce(&mut Wire) -> T) -> T {
    WIRE.with(|state| f(&mut state.borrow_mut()))
}
unsafe extern "system" fn buffer(command: i32, output: *mut u8) -> i32 {
    unsafe {
        std::ptr::write_bytes(output, 0, 257);
    }
    if command == 12 {
        unsafe {
            *output.add(1) = 1;
            *output.add(16) = 127;
            *output.add(18) = 50;
            *output.add(19) = 127;
        }
    }
    if command == 13 {
        unsafe {
            *output.add(12) = 2;
            *output.add(14) = wire(|w| w.fan_mode_bits);
            *output.add(50) = 3;
            for base in [16, 24] {
                let points = [50, 58, 60, 82, 80, 85, 100, 255];
                std::ptr::copy_nonoverlapping(points.as_ptr(), output.add(base), 8);
            }
        }
    }
    command
}
unsafe extern "system" fn settings(page: i32, offset: i32, length: i32, output: *mut u8) -> i32 {
    wire(|w| unsafe {
        std::ptr::copy_nonoverlapping(
            w.pages[page as usize][offset as usize..].as_ptr(),
            output,
            length as usize,
        )
    });
    4096
}
unsafe extern "system" fn send(command: i32, input: *const u8, length: i32) -> i32 {
    let payload = unsafe { std::slice::from_raw_parts(input, length as usize) };
    wire(|w| {
        w.writes.push((command, payload.to_vec()));
        match (command, payload) {
            (121, [value, 0, 0, 14]) => {
                if w.offset_command_failure == Some(*value) {
                    w.offset_command_failure = None;
                    return 0;
                }
                w.offset_raw = *value;
            }
            (121, [mode, 0, 0, 1]) => {
                w.fan_mode_bits = match mode {
                    0 => 0x02,
                    1 => 0x10,
                    2 => 0x08,
                    5 => 0x01,
                    6 => 0x04,
                    7 => 0x20,
                    8 => 0x40,
                    _ => return 0,
                };
            }
            (14, _) => return 0x14,
            _ => {}
        }
        command
    })
}
unsafe extern "system" fn save(page: i32, offset: i32, length: i32, input: *const u8) -> i32 {
    wire(|w| {
        w.pages[page as usize][offset as usize..offset as usize + length as usize]
            .copy_from_slice(unsafe { std::slice::from_raw_parts(input, length as usize) });
        if page == 4 && w.offset_save_failure == Some(offset) {
            w.offset_save_failure = None;
            return 0;
        }
        1
    })
}
unsafe extern "system" fn exchange(
    command: i32,
    input: *const u8,
    length: i32,
    output: *mut u8,
) -> i32 {
    unsafe {
        std::ptr::write_bytes(output, 0, 257);
    }
    wire(|w| unsafe {
        match *input {
            8 => {
                std::ptr::copy_nonoverlapping(w.pages[7].as_ptr(), output, 256);
                *output.add(256) = w.pages[7][0];
            }
            1 => {
                w.packets
                    .push(std::slice::from_raw_parts(input, length as usize).to_vec());
                *output = w.dust_reply_tag;
                *output.add(1) = w.running;
                *output.add(256) = w.dust_reply_tag;
            }
            3 => {
                *output.add(0xD7) = w.fan_mode_bits;
                *output.add(0xE7) = w.offset_raw;
            }
            5 => {
                *output.add(0x0A) = 2;
                for base in [0x0C, 0x1A] {
                    std::ptr::copy_nonoverlapping(
                        LIVE_CURVE.as_ptr(),
                        output.add(base),
                        LIVE_CURVE.len(),
                    );
                }
            }
            _ => return 0,
        }
        command
    })
}
unsafe extern "system" fn integer(command: i32, out: *mut i32) -> i32 {
    if command != 96 {
        return 0;
    }
    wire(|w| match w.wmi96 {
        Some(flags) => {
            unsafe { *out = flags };
            flags
        }
        None => 0,
    })
}
fn hal() -> Hal {
    wire(|w| *w = Wire::default());
    Hal::test_instance(
        Dchu::test_io(buffer, settings, send, save)
            .with_test_exchange(exchange)
            .with_test_integer(integer),
        None,
    )
}
#[test]
fn production_transport_uses_original_status_packet_and_start_command_without_cache_writes() {
    let hal = hal();
    let controller = Controller::default();
    let profile = controller.read(&hal).unwrap().unwrap();
    wire(|w| {
        assert!(w.writes.is_empty());
        assert_eq!(w.packets.len(), 1);
        assert_eq!(&w.packets[0][..7], &[1, 163, 0, 0, 0, 0, 184]);
        assert_eq!(w.packets[0].len(), 256);
    });
    controller.start(&hal, &profile.revision).unwrap();
    wire(|w| assert_eq!(w.writes, vec![(121, vec![1, 0, 0, 41])]));
    wire(|w| w.running = 7);
    assert!(controller.read(&hal).is_err());
    wire(|w| {
        w.running = 1;
        w.dust_reply_tag = 0;
        w.writes.clear();
    });
    assert!(controller.read(&hal).unwrap().unwrap().running);
    assert!(super::super::ensure_manual_allowed(&hal).is_err());
    wire(|w| assert!(w.writes.is_empty()));
}

#[test]
fn empty_capabilities_and_invalid_status_never_authorize_fan_or_dust_writes() {
    let hal = hal();
    let controller = Controller::default();
    let revision = controller.read(&hal).unwrap().unwrap().revision;
    for (version, tag, state) in [
        (0, 0xB8, 0),
        (1, 0, 0),
        (1, 0xB7, 0),
        (1, 0xB8, 2),
        (1, 0xB8, 0xA3),
        (1, 1, 0xA3),
    ] {
        wire(|w| {
            w.pages[7] = [0; 256];
            w.pages[7][0] = version;
            w.pages[7][10] = if version == 1 { 128 } else { 0 };
            w.wmi96 = Some(0);
            w.dust_reply_tag = tag;
            w.running = state;
        });
        assert!(controller.read(&hal).is_err());
        assert!(ensure_manual_allowed(&hal).is_err());
        assert!(ensure_software_allowed(&hal).is_err());
        assert!(crate::fanctl::set_mode(&hal, 0).is_err());
        assert!(controller.start(&hal, &revision).is_err());
        assert!(
            controller
                .apply(
                    &hal,
                    &Request {
                        expected_revision: revision.clone(),
                        schedule: disabled(),
                    }
                )
                .is_err()
        );
        assert!(controller.restore(&hal, &revision).is_err());
    }
    wire(|w| {
        assert!(w.writes.is_empty());
        assert_eq!(w.pages[4], [0; 256]);
    });
}

#[test]
fn version_zero_obeys_confirmed_wmi96_bit_seven() {
    let hal = hal();
    let controller = Controller::default();
    wire(|w| {
        w.pages[7] = [0; 256];
        w.pages[7][17] = 15;
        w.running = 1;
    });
    for flags in [None, Some(96), Some(-1)] {
        wire(|w| w.wmi96 = flags);
        assert!(ensure_manual_allowed(&hal).is_err());
    }
    wire(|w| {
        assert!(w.packets.is_empty());
        w.wmi96 = Some(0x80);
    });
    assert!(controller.read(&hal).unwrap().unwrap().running);
    assert!(ensure_manual_allowed(&hal).is_err());
    wire(|w| w.packets.clear());
    for flags in [0, 0x20] {
        wire(|w| w.wmi96 = Some(flags));
        assert!(controller.read(&hal).unwrap().is_none());
        ensure_manual_allowed(&hal).unwrap();
    }
    wire(|w| assert!(w.packets.is_empty() && w.writes.is_empty()));
}

#[test]
fn start_checks_instantaneous_fan_mode_even_when_saved_mode_differs() {
    let hal = hal();
    let controller = Controller::default();
    let idle = controller.read(&hal).unwrap().unwrap();
    wire(|w| w.fan_mode_bits = 0x10);
    let full_speed = controller.read(&hal).unwrap().unwrap();
    assert!(full_speed.start_blocked_reason.unwrap().contains("全速"));
    assert!(controller.start(&hal, &idle.revision).is_err());
    wire(|w| {
        assert!(w.writes.is_empty());
        w.fan_mode_bits = 0x02;
        w.pages[4][5] = 1;
    });
    controller.start(&hal, &idle.revision).unwrap();
}

#[test]
fn pending_software_release_blocks_dust_even_without_an_active_curve() {
    let hal = hal();
    let controller = Controller::default();
    let before = controller.read(&hal).unwrap().unwrap();
    hal.request_fan_recovery().unwrap();
    let pending = controller.read(&hal).unwrap().unwrap();
    assert!(pending.software_active);
    assert_ne!(before.revision, pending.revision);
    assert!(controller.start(&hal, &pending.revision).is_err());
    assert!(
        controller
            .apply(
                &hal,
                &Request {
                    expected_revision: pending.revision,
                    schedule: schedule(),
                }
            )
            .is_err()
    );
    wire(|w| assert!(w.writes.is_empty()));
}
#[path = "native_tests/offset.rs"]
mod offset;

#[test]
fn production_entrypoints_refuse_running_firmware_before_every_fan_or_mode_write() {
    let hal = hal();
    wire(|w| w.running = 1);
    let profile = crate::fanctl::read_profile(&hal).unwrap();
    assert!(profile.blocked_reason.unwrap().contains("除尘"));
    for result in [
        crate::fanctl::set_mode(&hal, 0).map(|_| ()),
        crate::fanctl::set_manual_offset(
            &hal,
            crate::fanctl::OffsetSource {
                mode: 0,
                slot: None,
            },
            0,
        ),
        crate::fanctl::apply_curve(&hal, None, None, None),
        crate::fanctl::restore_initial(&hal),
        crate::fanctl::restore_default(&hal),
        crate::fanctl::rpm::apply(&hal, &Default::default()),
        crate::fanctl::rpm::restore_default(&hal),
        crate::fanctl::software::apply(&hal, &Default::default()),
        crate::powerctl::mode::checkpoint(&hal, 2).map(|_| ()),
    ] {
        assert!(result.unwrap_err().to_string().contains("除尘"));
    }
    wire(|w| assert!(w.writes.is_empty()));
}
#[test]
fn software_interlock_requires_an_explicitly_disabled_plan_and_never_uses_other_feature_bits() {
    let hal = hal();
    assert!(super::super::ensure_software_allowed(&hal).is_err());
    wire(|w| w.pages[4][81..86].copy_from_slice(&disabled().saved()));
    super::super::ensure_software_allowed(&hal).unwrap();
    wire(|w| w.pages[4][82] = 128);
    assert!(super::super::ensure_software_allowed(&hal).is_err());
    wire(|w| {
        w.pages[7][10] = 0;
        w.pages[7][6] = 128;
        w.running = 7;
    });
    super::super::ensure_software_allowed(&hal).unwrap();
}
#[test]
fn production_calibration_interlock_blocks_start_and_schedule_before_command_transport() {
    let mut hal = hal();
    let controller = Controller::default();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("calibration.json");
    std::fs::write(&path, b"{broken}").unwrap();
    hal.bind_calibration_record(path);
    let profile = controller.read(&hal).unwrap().unwrap();
    assert!(profile.start_blocked_reason.is_some());
    assert!(controller.start(&hal, &profile.revision).is_err());
    assert!(
        controller
            .apply(
                &hal,
                &Request {
                    expected_revision: profile.revision.clone(),
                    schedule: schedule()
                }
            )
            .is_err()
    );
    assert!(controller.restore(&hal, &profile.revision).is_err());
    wire(|w| assert!(w.writes.is_empty()));
}
