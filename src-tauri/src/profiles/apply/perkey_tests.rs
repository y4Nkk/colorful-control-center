use super::*;
use crate::lighting::perkey;
use std::sync::atomic::Ordering;
#[path = "perkey_fixture.rs"]
pub(super) mod fixture;
use fixture::*;
#[path = "perkey_session_tests.rs"]
mod session_tests;

#[test]
fn capture_roundtrip_disabled_nonzero_perkey_is_read_only_and_starts_no_events() {
    let h = Harness::new();
    h.f.device.0.lock().unwrap().pages[2][80] = 0;
    let captured = crate::profiles::capture_current(&h.f.hal, &h.f.gpu, h.f.devices()).unwrap();
    assert!(captured.keyboard_lighting.is_none());
    let target = captured.perkey_lighting.as_ref().unwrap();
    assert!(!target.values.enabled);
    assert_eq!(target.values.brightness, 4);
    assert_eq!(target.device, *h.io.info.lock().unwrap());
    assert!(
        matches!(&target.values.pattern, perkey::Pattern::Static { colors } if colors.len() == 120)
    );
    let dir = tempfile::tempdir().unwrap();
    crate::profiles::save_in(dir.path(), captured.clone(), "逐键").unwrap();
    let file = crate::profiles::load_from(dir.path(), "逐键").unwrap();
    assert!(file.settings == captured);
    assert!(h.f.device.0.lock().unwrap().events.is_empty());
    assert_eq!(h.runtime.prepared.load(Ordering::SeqCst), 0);
    assert!(h.io.traffic.lock().unwrap().packets.is_empty());
}

#[test]
fn strict_shape_mcu_type_and_mutual_exclusion_fail_before_cpu_or_led_writes() {
    let h = Harness::new();
    let mut invalid = vec![];
    let mut item = h.settings(false);
    if let perkey::Pattern::Static { colors } =
        &mut item.perkey_lighting.as_mut().unwrap().values.pattern
    {
        colors.pop();
    }
    invalid.push(item);
    let mut item = h.settings(true);
    item.perkey_lighting.as_mut().unwrap().keyboard_type = 6;
    invalid.push(item);
    let mut item = h.settings(false);
    item.perkey_lighting.as_mut().unwrap().device.new_effects = false;
    invalid.push(item);
    let mut item = h.settings(false);
    item.perkey_lighting.as_mut().unwrap().values.speed = 3;
    invalid.push(item);
    let mut item = h.settings(false);
    item.keyboard_lighting = Some(serde_json::from_value(serde_json::json!({"keyboardType":1,"enabled":true,"settings":{"family":"white","brightness":3}})).unwrap());
    invalid.push(item);
    let mut item = h.settings(false);
    item.lighting_options = Some(serde_json::from_value(serde_json::json!({"keyboardType":6,"values":{"idleTimer":{"enabled":true,"seconds":60},"bootOverride":null,"logo":null,"lightBar":null}})).unwrap());
    invalid.push(item);
    for item in invalid {
        assert!(h.apply(&item).is_err());
    }
    let mut legacy = h.target(false);
    legacy.device.new_effects = false;
    legacy.values.pattern = perkey::Pattern::Wave {
        direction: 7,
        color: None,
    };
    assert!(legacy.validate().is_err());
    let mut json = serde_json::to_value(h.target(false)).unwrap();
    json["device"]["obsolete"] = serde_json::Value::Bool(true);
    assert!(serde_json::from_value::<perkey::Target>(json).is_err());
    assert!(h.f.device.0.lock().unwrap().events.is_empty());
    assert!(h.io.traffic.lock().unwrap().packets.is_empty());
}

#[test]
fn later_gpu_failure_restores_pages_and_first_baseline_is_shared_with_single_restore() {
    let h = Harness::new();
    let original = h.pages();
    let mut wanted = h.settings(false);
    failing_gpu(&mut wanted);
    h.f.device.0.lock().unwrap().fail_gpu = true;
    let steps = h.apply(&wanted).unwrap();
    assert!(
        steps
            .iter()
            .any(|step| step.step == "恢复逐键灯效" && step.ok)
    );
    assert_eq!(h.pages(), original);
    h.f.device.0.lock().unwrap().fail_gpu = false;
    assert!(
        h.apply(&h.settings(false))
            .unwrap()
            .iter()
            .all(|step| step.ok)
    );
    h.f.device.0.lock().unwrap().pages[2][39] = 90;
    h.f.device.0.lock().unwrap().pages[2][85] = 222;
    h.f.perkey.restore(&h.f.hal).unwrap();
    let mut expected = original;
    expected[0][39] = 90;
    expected[0][85] = 222;
    assert_eq!(h.pages(), expected);
    assert!(!h.f.perkey.read(&h.f.hal).unwrap().can_restore);
}

#[test]
fn cross_page_partial_save_is_compensated_before_later_modules() {
    let h = Harness::new();
    let original = h.pages();
    h.io.traffic.lock().unwrap().fail_writes = vec![6];
    let mut wanted = h.settings(false);
    failing_gpu(&mut wanted);
    let steps = h.apply(&wanted).unwrap();
    assert!(steps.iter().any(|step| step.step == "逐键灯效" && !step.ok));
    assert!(!steps.iter().any(|step| step.step == "GPU 偏移"));
    assert_eq!(h.pages(), original);
    assert!(h.f.perkey.read(&h.f.hal).unwrap().can_restore);
}

#[test]
fn perkey_and_options_merge_shared_page_and_both_restore_after_gpu_failure() {
    let h = Harness::new();
    let original = h.pages();
    let mut wanted = h.settings(false);
    failing_gpu(&mut wanted);
    wanted.lighting_options = Some(serde_json::from_value(serde_json::json!({"keyboardType":3,"values":{"idleTimer":{"enabled":true,"seconds":65},"bootOverride":null,"logo":null,"lightBar":null}})).unwrap());
    h.f.device.0.lock().unwrap().fail_gpu = true;
    let steps = h.apply(&wanted).unwrap();
    assert!(
        steps
            .iter()
            .any(|step| step.step == "灯光策略与外围灯区" && step.ok)
    );
    assert!(
        steps
            .iter()
            .any(|step| step.step == "恢复灯光策略与外围灯区" && step.ok)
    );
    assert_eq!(h.pages(), original);
    h.f.device.0.lock().unwrap().fail_gpu = false;
    assert!(h.apply(&wanted).unwrap().iter().all(|step| step.ok));
    assert_eq!(&h.pages()[0][37..40], &[0, 1, 5]);
    assert_eq!(
        h.f.perkey.read(&h.f.hal).unwrap().current,
        wanted.perkey_lighting.unwrap().values
    );
}

#[test]
fn missing_test_transports_refuse_without_native_fallback() {
    let mut h = Harness::new();
    let wanted = h.settings(true);
    h.f.perkey = perkey::Controller::default();
    assert!(
        h.apply(&wanted)
            .err()
            .unwrap()
            .to_string()
            .contains("未访问真实HID")
    );
    h.f.perkey = perkey::Controller::default().with_test_io(h.io.clone());
    assert!(
        h.apply(&wanted)
            .err()
            .unwrap()
            .to_string()
            .contains("未启动真实钩子")
    );
    assert!(h.f.device.0.lock().unwrap().events.is_empty());
    assert!(h.io.traffic.lock().unwrap().packets.is_empty());
}
