use super::*;
use crate::lighting;
fn setup(kind: u8) -> Fixture {
    let mut f = Fixture::new();
    f.hal.test_manufacturer = Some("Notebook".into());
    {
        let mut s = f.device.0.lock().unwrap();
        s.bios[0] = 1;
        s.bios[1] = 0;
        s.pages[2][4] = kind;
        s.pages[2][32] = 8;
        s.pages[2][35] = 3;
        s.pages[2][80] = 1;
        s.pages[2][81..84].copy_from_slice(&[10, 20, 30]);
        s.pages[2][127] = 1;
    }
    f
}
#[test]
fn color_cycle_capture_and_profile_rollback_share_initial_lighting_baseline() {
    let f = setup(6);
    let initial = f.device.0.lock().unwrap().pages[2];
    let identity = f.lighting.color_cycle_identity(&f.hal).unwrap();
    assert_eq!(f.lighting.cycle_color(&f.hal, &identity).unwrap().index, 2);
    let captured = crate::profiles::capture_current(&f.hal, &f.gpu, f.devices()).unwrap();
    let mut target = captured.keyboard_lighting.unwrap();
    let lighting::LightingRequest::ZoneRgb { all_color, .. } = &mut target.settings else {
        panic!()
    };
    assert_eq!([all_color.r, all_color.g, all_color.b], [0, 255, 0]);
    all_color.r = 4;
    all_color.g = 5;
    all_color.b = 6;
    let settings = ProfileSettings {
        keyboard_lighting: Some(target),
        gpu: Some(crate::gpuctl::GpuClockSettings {
            core_offset_mhz: Some(60),
            mem_offset_mhz: None,
        }),
        ..Default::default()
    };
    f.device.0.lock().unwrap().fail_gpu = true;
    let steps = apply_settings(&f.hal, &f.gpu, f.devices(), &settings).unwrap();
    assert!(steps.iter().any(|s| s.step == "GPU 偏移" && !s.ok));
    assert_eq!(&f.device.0.lock().unwrap().pages[2][81..84], &[0, 255, 0]);
    assert_eq!(f.device.0.lock().unwrap().pages[2][127], 2);
    assert!(f.lighting.restore(&f.hal).ok);
    assert_eq!(f.device.0.lock().unwrap().pages[2], initial);
}
#[test]
fn cycle_changes_only_all_color_and_cursor_for_each_original_keyboard_type() {
    for kind in [6, 22, 38] {
        let f = setup(kind);
        let mut expected = f.device.0.lock().unwrap().pages[2];
        expected[81..84].copy_from_slice(&[0, 255, 0]);
        expected[127] = 2;
        let id = f.lighting.color_cycle_identity(&f.hal).unwrap();
        f.lighting.cycle_color(&f.hal, &id).unwrap();
        assert_eq!(f.device.0.lock().unwrap().pages[2], expected);
        let words = f.device.0.lock().unwrap().events.clone();
        assert_eq!(
            words
                .iter()
                .filter(|w| w.starts_with("LIGHT:103:f"))
                .count(),
            3
        );
        assert!(
            !words
                .iter()
                .any(|w| w.starts_with("LIGHT:103:e") || w.starts_with("LIGHT:103:f4"))
        );
    }
}
#[test]
fn unknown_cursor_or_missing_test_identity_never_falls_back_to_hardware() {
    let mut f = setup(6);
    f.hal.test_manufacturer = None;
    assert!(f.lighting.color_cycle_identity(&f.hal).is_err());
    assert!(f.device.0.lock().unwrap().events.is_empty());
    f.hal.test_manufacturer = Some("Notebook".into());
    f.device.0.lock().unwrap().pages[2][127] = 254;
    assert!(f.lighting.color_cycle_identity(&f.hal).is_err());
    assert!(f.lighting.cycle_color(&f.hal, "stale").is_err());
    assert!(f.device.0.lock().unwrap().events.is_empty());
}
#[test]
fn changed_palette_or_device_rejects_old_authorization_without_output() {
    let mut f = setup(6);
    let id = f.lighting.color_cycle_identity(&f.hal).unwrap();
    f.hal.test_manufacturer = Some("MONSTER".into());
    assert!(f.lighting.cycle_color(&f.hal, &id).is_err());
    assert!(f.device.0.lock().unwrap().events.is_empty());
    f.hal.test_manufacturer = Some("Notebook".into());
    f.device.0.lock().unwrap().pages[2][4] = 22;
    assert!(f.lighting.cycle_color(&f.hal, &id).is_err());
    assert!(f.device.0.lock().unwrap().events.is_empty());
}
