use super::perkey_tests::fixture::Harness;
use super::*;
use crate::lighting;

fn ordinary(kind: u8) -> Fixture {
    let f = Fixture::new();
    {
        let mut data = f.device.0.lock().unwrap();
        data.pages[2][4] = kind;
        data.bios[0] = 1;
        data.pages[2][32] = 1;
        data.pages[2][35] = 3;
        data.pages[2][80] = 1;
        for at in [81, 85, 89, 93] {
            data.pages[2][at..at + 3].copy_from_slice(&[10, 20, 30]);
        }
    }
    f
}
#[test]
fn ordinary_and_white_toggle_preserve_levels_and_exact_first_restore() {
    for kind in [1, 2, 6, 22, 38] {
        let f = ordinary(kind);
        let before = f.device.0.lock().unwrap().pages[2];
        let identity = lighting::hotkey::identity(&f.lighting.read_profile(&f.hal)).unwrap();
        assert!(!f.lighting.toggle_enabled(&f.hal, &identity).unwrap());
        assert_eq!(f.device.0.lock().unwrap().pages[2][35], 3);
        assert!(f.lighting.toggle_enabled(&f.hal, &identity).unwrap());
        assert_eq!(f.device.0.lock().unwrap().pages[2], before);
        assert!(f.lighting.restore(&f.hal).ok);
        assert_eq!(f.device.0.lock().unwrap().pages[2], before);
    }
}
#[test]
fn ordinary_toggle_failure_recovers_and_changed_type_has_no_writes() {
    let f = ordinary(2);
    let identity = lighting::hotkey::identity(&f.lighting.read_profile(&f.hal)).unwrap();
    let before = f.device.0.lock().unwrap().pages[2];
    f.device.0.lock().unwrap().fail_lighting_words = 1;
    assert!(f.lighting.toggle_enabled(&f.hal, &identity).is_err());
    assert_eq!(f.device.0.lock().unwrap().pages[2], before);
    assert!(f.lighting.restore(&f.hal).ok);
    {
        let mut data = f.device.0.lock().unwrap();
        data.pages[2][4] = 6;
        data.events.clear();
    }
    assert!(f.lighting.toggle_enabled(&f.hal, &identity).is_err());
    assert!(f.device.0.lock().unwrap().events.is_empty());
}
#[test]
fn perkey_toggle_suspends_and_resumes_reactive_session_without_losing_brightness() {
    let h = Harness::new();
    h.start_old();
    let before = h.f.perkey.read(&h.f.hal).unwrap().current;
    let identity = h.f.perkey.hotkey_identity(&h.f.hal).unwrap();
    assert!(!h.f.perkey.toggle_enabled(&h.f.hal, &identity).unwrap());
    assert_eq!(h.runtime.active(), 0);
    let off = h.f.perkey.read(&h.f.hal).unwrap().current;
    assert_eq!(off.brightness, before.brightness);
    assert_eq!(off.pattern, before.pattern);
    assert!(h.f.perkey.toggle_enabled(&h.f.hal, &identity).unwrap());
    assert_eq!(h.runtime.active(), 1);
    assert_eq!(h.f.perkey.read(&h.f.hal).unwrap().current, before);
}
#[test]
fn perkey_partial_toggle_recovers_session_and_replaced_device_never_writes() {
    let h = Harness::new();
    h.start_old();
    let identity = h.f.perkey.hotkey_identity(&h.f.hal).unwrap();
    let pages = h.pages();
    let next = h.io.traffic.lock().unwrap().packets.len() + 1;
    h.io.traffic.lock().unwrap().fail_sends.push(next);
    assert!(h.f.perkey.toggle_enabled(&h.f.hal, &identity).is_err());
    assert_eq!(h.pages(), pages);
    assert_eq!(h.runtime.active(), 1);
    *h.io.identity.lock().unwrap() = "replacement".into();
    let count = h.io.traffic.lock().unwrap().packets.len();
    assert!(h.f.perkey.toggle_enabled(&h.f.hal, &identity).is_err());
    assert_eq!(h.io.traffic.lock().unwrap().packets.len(), count);
}

#[test]
fn zone_keys_change_only_the_selected_flag_and_share_exact_initial_restore() {
    for master in [false, true] {
        for zone in 0..3 {
            let f = ordinary(2);
            {
                let mut state = f.device.0.lock().unwrap();
                state.pages[2][80] = u8::from(master);
                for offset in [84, 88, 92] {
                    state.pages[2][offset] = 1;
                }
            }
            let before = f.device.0.lock().unwrap().pages[2];
            let identity =
                lighting::zone_hotkey::identity(&f.lighting.read_profile(&f.hal)).unwrap();
            assert!(!f.lighting.toggle_zone(&f.hal, &identity, zone).unwrap());
            let event = f
                .device
                .0
                .lock()
                .unwrap()
                .events
                .iter()
                .rev()
                .find(|event| event.starts_with("LIGHT:103:e0"))
                .cloned()
                .unwrap();
            let word = u32::from_str_radix(event.rsplit(':').next().unwrap(), 16).unwrap();
            assert_eq!(word & (1 << 12) != 0, master);
            assert_eq!((word >> 16) & 7, if master { 7 ^ (1 << zone) } else { 0 });
            let mut expected = before;
            expected[84 + usize::from(zone) * 4] = 0;
            assert_eq!(f.device.0.lock().unwrap().pages[2], expected);
            let captured = crate::profiles::capture_current(&f.hal, &f.gpu, f.devices(), &crate::profiles::ProfileGroup::ALL).unwrap();
            let target = captured.keyboard_lighting.unwrap();
            assert_eq!(target.enabled, master);
            let crate::lighting::LightingRequest::ZoneRgb { zones, .. } = target.settings else {
                panic!()
            };
            assert!(!zones[usize::from(zone)].enabled);
            assert!(f.lighting.toggle_zone(&f.hal, &identity, zone).unwrap());
            assert_eq!(f.device.0.lock().unwrap().pages[2], before);
            assert!(f.lighting.restore(&f.hal).ok);
            assert_eq!(f.device.0.lock().unwrap().pages[2], before);
        }
    }
}

#[test]
fn zone_keys_reject_other_original_types_and_invalid_region_without_output() {
    for kind in [1, 3, 6, 22, 23, 38] {
        let f = ordinary(kind);
        assert!(lighting::zone_hotkey::identity(&f.lighting.read_profile(&f.hal)).is_err());
        f.device.0.lock().unwrap().events.clear();
        assert!(f.lighting.toggle_zone(&f.hal, "stale", 0).is_err());
        assert!(f.device.0.lock().unwrap().events.is_empty());
    }
    let f = ordinary(2);
    let id = lighting::zone_hotkey::identity(&f.lighting.read_profile(&f.hal)).unwrap();
    assert!(f.lighting.toggle_zone(&f.hal, &id, 3).is_err());
    assert!(f.device.0.lock().unwrap().events.is_empty());
}

#[test]
fn zone_key_failure_compensates_and_replaced_type_never_uses_old_authorization() {
    let f = ordinary(2);
    let before = f.device.0.lock().unwrap().pages[2];
    let id = lighting::zone_hotkey::identity(&f.lighting.read_profile(&f.hal)).unwrap();
    f.device.0.lock().unwrap().fail_lighting_words = 1;
    assert!(f.lighting.toggle_zone(&f.hal, &id, 1).is_err());
    assert_eq!(f.device.0.lock().unwrap().pages[2], before);
    assert!(f.lighting.restore(&f.hal).ok);
    {
        let mut state = f.device.0.lock().unwrap();
        state.pages[2][4] = 6;
        state.events.clear();
    }
    assert!(f.lighting.toggle_zone(&f.hal, &id, 1).is_err());
    assert!(f.device.0.lock().unwrap().events.is_empty());
}

#[test]
fn zone_key_save_failure_restores_command_state_and_first_snapshot() {
    let f = ordinary(2);
    let before = f.device.0.lock().unwrap().pages[2];
    let id = lighting::zone_hotkey::identity(&f.lighting.read_profile(&f.hal)).unwrap();
    f.device.0.lock().unwrap().fail_page2_at = Some(88);
    assert!(f.lighting.toggle_zone(&f.hal, &id, 1).is_err());
    assert_eq!(f.device.0.lock().unwrap().pages[2], before);
    assert!(f.lighting.restore(&f.hal).ok);
    assert_eq!(f.device.0.lock().unwrap().pages[2], before);
}
