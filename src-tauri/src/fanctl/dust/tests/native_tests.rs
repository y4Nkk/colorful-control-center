use super::*;
use crate::hal::oem_dchu::Dchu;

struct Wire {
    pages: [[u8; 256]; 8],
    running: u8,
    packets: Vec<Vec<u8>>,
    writes: Vec<(i32, Vec<u8>)>,
}
impl Default for Wire {
    fn default() -> Self {
        let mut pages = [[0; 256]; 8];
        pages[7][0] = 1;
        pages[7][10] = 128;
        Self {
            pages,
            running: 0,
            packets: vec![],
            writes: vec![],
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
            *output.add(19) = 127;
        }
    }
    if command == 13 {
        unsafe {
            *output.add(12) = 2;
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
    wire(|w| {
        w.writes.push((
            command,
            unsafe { std::slice::from_raw_parts(input, length as usize) }.to_vec(),
        ))
    });
    command
}
unsafe extern "system" fn save(page: i32, offset: i32, length: i32, input: *const u8) -> i32 {
    wire(|w| {
        w.pages[page as usize][offset as usize..offset as usize + length as usize]
            .copy_from_slice(unsafe { std::slice::from_raw_parts(input, length as usize) })
    });
    1
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
    wire(|w| {
        if unsafe { *input } == 8 {
            unsafe {
                std::ptr::copy_nonoverlapping(w.pages[7].as_ptr(), output, 256);
                *output.add(256) = w.pages[7][0];
            }
            return;
        }
        w.packets
            .push(unsafe { std::slice::from_raw_parts(input, length as usize) }.to_vec());
        unsafe {
            *output.add(1) = w.running;
        }
    });
    command
}
fn hal() -> Hal {
    wire(|w| *w = Wire::default());
    Hal::test_instance(
        Dchu::test_io(buffer, settings, send, save).with_test_exchange(exchange),
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
        w.writes.clear();
    });
    assert!(super::super::ensure_manual_allowed(&hal).is_err());
    wire(|w| assert!(w.writes.is_empty()));
}
#[test]
fn production_entrypoints_refuse_running_firmware_before_every_fan_or_mode_write() {
    let hal = hal();
    wire(|w| w.running = 1);
    let profile = crate::fanctl::read_profile(&hal).unwrap();
    assert!(profile.blocked_reason.unwrap().contains("除尘"));
    for result in [
        crate::fanctl::set_mode(&hal, 0).map(|_| ()),
        crate::fanctl::set_offset(&hal, 0),
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
