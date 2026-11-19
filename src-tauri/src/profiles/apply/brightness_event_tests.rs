use super::*;
use crate::lighting::LightingController;
use std::cell::Cell;
thread_local! {
    static WHITE_REPORT: Cell<i32> = const { Cell::new(2) };
    static WHITE_READS: Cell<usize> = const { Cell::new(0) };
}

fn setup(kind: u8) -> Fixture {
    let f = Fixture::new();
    {
        let mut s = f.device.0.lock().unwrap();
        s.bios[0] = 1;
        s.bios[1] = 0;
        s.pages[2][4] = kind;
        s.pages[2][32] = 8;
        s.pages[2][35] = 4;
        s.pages[2][80] = 1;
        s.pages[2][81..84].copy_from_slice(&[10, 20, 30]);
    }
    f
}
unsafe extern "system" fn integer(command: i32, output: *mut i32) -> i32 {
    if command != 61 {
        return 0;
    }
    WHITE_READS.with(|reads| reads.set(reads.get() + 1));
    let value = WHITE_REPORT.get();
    if value == i32::MIN {
        return 0;
    }
    unsafe {
        output.write(value);
    }
    value
}
fn white_hal() -> crate::hal::Hal {
    crate::hal::Hal::test_instance(fixtures::io().with_test_integer(integer), None)
}

#[test]
fn rgb_event_capture_and_later_profile_failure_preserve_shared_first_baseline() {
    let f = setup(22);
    let initial = f.device.0.lock().unwrap().pages[2];
    let identity = f.lighting.brightness_event_identity(&f.hal).unwrap();
    let off = f
        .lighting
        .brightness_event(&f.hal, &identity)
        .unwrap()
        .unwrap();
    assert_eq!(off.level, 0);
    let captured = crate::profiles::capture_current(&f.hal, &f.gpu, f.devices(), &crate::profiles::ProfileGroup::ALL).unwrap();
    let mut target = captured.keyboard_lighting.unwrap();
    assert!(!target.enabled);
    let crate::lighting::LightingRequest::ZoneRgb { brightness, .. } = &mut target.settings else {
        panic!()
    };
    assert_eq!(*brightness, 4);
    *brightness = 2;
    target.enabled = true;
    let settings = ProfileSettings {
        keyboard_lighting: Some(target),
        gpu: Some(crate::gpuctl::GpuClockSettings {
            core_offset_mhz: Some(60),
            mem_offset_mhz: None,
        }),
        ..Default::default()
    };
    let roundtrip: ProfileSettings =
        serde_json::from_slice(&serde_json::to_vec(&settings).unwrap()).unwrap();
    assert!(roundtrip == settings);
    f.device.0.lock().unwrap().fail_gpu = true;
    let steps = apply_settings(&f.hal, &f.gpu, f.devices(), &settings).unwrap();
    assert!(steps.iter().any(|s| s.step == "GPU 偏移" && !s.ok));
    assert_eq!(f.device.0.lock().unwrap().pages[2][35], 4);
    assert_eq!(f.device.0.lock().unwrap().pages[2][80], 0);
    assert!(f.lighting.restore(&f.hal).ok);
    assert_eq!(f.device.0.lock().unwrap().pages[2], initial);
}

#[test]
fn rgb_type_change_rejects_old_event_permission_without_any_output() {
    let f = setup(22);
    let identity = f.lighting.brightness_event_identity(&f.hal).unwrap();
    f.device.0.lock().unwrap().pages[2][4] = 6;
    assert!(f.lighting.brightness_event(&f.hal, &identity).is_err());
    assert!(f.device.0.lock().unwrap().events.is_empty());
    assert!(f.lighting.read_profile(&f.hal).baseline.is_none());
}

#[test]
fn valid_integer_abi_reports_sync_white_cache_without_device_commands_or_initial_snapshot() {
    for (six, levels) in [(false, 2), (true, 5)] {
        let f = setup(1);
        if six {
            f.device.0.lock().unwrap().bios[7] |= 64;
        }
        let hal = white_hal();
        let controller = LightingController::default();
        WHITE_READS.set(0);
        let identity = controller.brightness_event_identity(&hal).unwrap();
        assert_eq!(
            WHITE_READS.get(),
            0,
            "configuration reads never consume a brightness report"
        );
        for level in 0..=5 {
            WHITE_REPORT.set(level);
            let result = controller
                .brightness_event(&hal, &identity)
                .unwrap()
                .unwrap();
            assert!(result.firmware_report);
            assert_eq!(result.level, level as u8);
            assert_eq!(result.display_maximum, levels as u8);
            assert_eq!(result.show_osd(), level <= levels);
            let s = f.device.0.lock().unwrap();
            assert_eq!(s.pages[2][35], level as u8);
            assert_eq!(s.pages[2][80], u8::from(level != 0));
            assert!(!s.events.iter().any(|event| event.starts_with("LIGHT:")));
        }
        assert!(controller.read_profile(&hal).baseline.is_none());
        assert!(!controller.restore(&hal).ok);
    }
}

#[test]
fn integer_echo_failure_and_out_of_range_never_overwrite_white_report_cache() {
    for raw in [61, i32::MIN, -1, 6] {
        let f = setup(1);
        let hal = white_hal();
        let controller = LightingController::default();
        let before = f.device.0.lock().unwrap().pages[2];
        let identity = controller.brightness_event_identity(&hal).unwrap();
        WHITE_REPORT.set(raw);
        assert!(controller.brightness_event(&hal, &identity).is_err());
        assert_eq!(f.device.0.lock().unwrap().pages[2], before);
        assert!(f.device.0.lock().unwrap().events.is_empty());
        assert!(controller.read_profile(&hal).baseline.is_none());
    }
}

#[test]
fn white_capability_change_revokes_permission_before_report_read_or_cache_write() {
    let f = setup(1);
    let hal = white_hal();
    let controller = LightingController::default();
    let identity = controller.brightness_event_identity(&hal).unwrap();
    WHITE_READS.set(0);
    f.device.0.lock().unwrap().bios[7] |= 64;
    assert!(controller.brightness_event(&hal, &identity).is_err());
    assert_eq!(WHITE_READS.get(), 0);
    assert!(f.device.0.lock().unwrap().events.is_empty());
    f.device.0.lock().unwrap().bios[0] = 0;
    assert!(controller.brightness_event_identity(&hal).is_err());
}
