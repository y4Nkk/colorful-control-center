use super::*;
#[path = "multizone_application_tests.rs"]
mod application;
#[path = "multizone_fixture.rs"]
mod fixture;

#[test]
fn capture_roundtrip_selects_multizone_and_preserves_options_owner() {
    let mut f = Fixture::new();
    let io = fixture::enable(&mut f);
    let settings = crate::profiles::capture_current(&f.hal, &f.gpu, f.devices(), &crate::profiles::ProfileGroup::ALL).unwrap();
    assert!(settings.multi_zone_lighting.is_some());
    assert!(settings.keyboard_lighting.is_none() && settings.perkey_lighting.is_none());
    assert_eq!(
        settings.lighting_options.as_ref().unwrap().keyboard_type,
        23
    );
    assert!(io.traffic.lock().unwrap().packets.is_empty());
    let raw = serde_json::to_string(&settings).unwrap();
    assert!(serde_json::from_str::<ProfileSettings>(&raw).unwrap() == settings);
}
#[test]
fn successful_profile_and_single_restore_share_first_baseline() {
    let mut f = Fixture::new();
    fixture::enable(&mut f);
    let original = f.multi_zone.read(&f.hal).unwrap().unwrap().current.unwrap();
    let steps = apply_settings(
        &f.hal,
        &f.gpu,
        f.devices(),
        &ProfileSettings {
            multi_zone_lighting: Some(fixture::target(&f)),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(steps.iter().all(|step| step.ok));
    let profile = f.multi_zone.read(&f.hal).unwrap().unwrap();
    assert!(profile.can_restore && profile.initial == Some(original.clone()));
    f.multi_zone.restore(&f.hal, &profile.revision).unwrap();
    assert!(f.multi_zone.read(&f.hal).unwrap().unwrap().current == Some(original));
}
#[test]
fn later_gpu_failure_restores_multizone_and_preserves_unowned_bytes() {
    let mut f = Fixture::new();
    fixture::enable(&mut f);
    let before = f.multi_zone.read(&f.hal).unwrap().unwrap().current.unwrap();
    {
        let mut data = f.device.0.lock().unwrap();
        data.fail_gpu = true;
        data.pages[2][100] = 77;
    }
    let steps = apply_settings(
        &f.hal,
        &f.gpu,
        f.devices(),
        &ProfileSettings {
            multi_zone_lighting: Some(fixture::target(&f)),
            gpu: Some(crate::gpuctl::GpuClockSettings {
                core_offset_mhz: Some(60),
                mem_offset_mhz: None,
            }),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(steps.iter().any(|step| !step.ok && step.step == "GPU 偏移"));
    assert!(
        steps
            .iter()
            .any(|step| step.ok && step.step == "恢复多区键盘灯效")
    );
    assert!(f.multi_zone.read(&f.hal).unwrap().unwrap().current == Some(before));
    assert_eq!(f.device.0.lock().unwrap().pages[2][100], 77);
}
#[test]
fn source_change_and_missing_explicit_io_refuse_all_other_writes() {
    for missing in [false, true] {
        let mut f = Fixture::new();
        let io = fixture::enable(&mut f);
        let target = fixture::target(&f);
        if missing {
            f.multi_zone = Default::default();
        } else {
            f.device.0.lock().unwrap().bios[23] = 0;
        }
        assert!(
            apply_settings(
                &f.hal,
                &f.gpu,
                f.devices(),
                &ProfileSettings {
                    multi_zone_lighting: Some(target),
                    pl1_w: Some(77.0),
                    ..Default::default()
                }
            )
            .is_err()
        );
        assert!(io.traffic.lock().unwrap().packets.is_empty());
        assert!(f.device.0.lock().unwrap().events.is_empty());
    }
}
#[test]
fn local_failure_and_failed_compensation_keep_retryable_baseline() {
    let mut f = Fixture::new();
    let io = fixture::enable(&mut f);
    let target = fixture::target(&f);
    io.traffic.lock().unwrap().fail_sends = vec![1, 2, 3];
    let steps = apply_settings(
        &f.hal,
        &f.gpu,
        f.devices(),
        &ProfileSettings {
            multi_zone_lighting: Some(target),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(
        steps
            .iter()
            .any(|step| !step.ok && step.step == "恢复多区键盘灯效")
    );
    let profile = f.multi_zone.read(&f.hal).unwrap().unwrap();
    assert!(profile.can_restore);
    f.multi_zone.restore(&f.hal, &profile.revision).unwrap();
}
#[test]
fn options_and_multizone_transactions_keep_independent_fields() {
    let mut f = Fixture::new();
    fixture::enable(&mut f);
    let options = f.lighting_options.read(&f.hal).unwrap();
    let mut values = options.current;
    values.idle_timer.enabled = true;
    values.idle_timer.seconds = 120;
    let steps = apply_settings(
        &f.hal,
        &f.gpu,
        f.devices(),
        &ProfileSettings {
            multi_zone_lighting: Some(fixture::target(&f)),
            lighting_options: Some(crate::lighting::options::Target {
                keyboard_type: 23,
                values: values.clone(),
            }),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(steps.iter().all(|step| step.ok));
    assert_eq!(f.lighting_options.read(&f.hal).unwrap().current, values);
    assert_eq!(
        f.multi_zone
            .read(&f.hal)
            .unwrap()
            .unwrap()
            .current
            .unwrap()
            .mode,
        4
    );
}
#[test]
fn approved_brightness_uses_current_domain_and_preserves_other_values() {
    let mut f = Fixture::new();
    fixture::enable(&mut f);
    let identity = f.multi_zone.hotkey_identity(&f.hal).unwrap();
    let before = f.multi_zone.read(&f.hal).unwrap().unwrap().current.unwrap();
    assert_eq!(
        f.multi_zone
            .adjust_brightness(&f.hal, &identity, -1)
            .unwrap(),
        3
    );
    let current = f.multi_zone.read(&f.hal).unwrap().unwrap().current.unwrap();
    assert_eq!(current.zones, before.zones);
    f.device.0.lock().unwrap().bios[23] = 0;
    assert!(
        f.multi_zone
            .adjust_brightness(&f.hal, &identity, 1)
            .is_err()
    );
}
#[test]
fn profile_validation_rejects_conflicting_keyboard_families() {
    let mut f = Fixture::new();
    fixture::enable(&mut f);
    let mut settings = ProfileSettings {
        multi_zone_lighting: Some(fixture::target(&f)),
        ..Default::default()
    };
    settings.keyboard_lighting = Some(crate::lighting::Target {
        keyboard_type: 1,
        enabled: true,
        settings: crate::lighting::LightingRequest::White { brightness: 1 },
    });
    assert!(crate::profiles::validate(&settings).is_err());
}

#[test]
fn independent_cover_follows_multizone_on_apply_and_later_failure_restore() {
    for fail_gpu in [false, true] {
        let mut f = Fixture::new();
        fixture::enable(&mut f);
        {
            let mut data = f.device.0.lock().unwrap();
            data.bios[23] |= 64;
            data.pages[2][100..104].copy_from_slice(&[3, 10, 20, 30]);
            data.fail_gpu = fail_gpu;
        }
        let initial = f.lighting_options.read(&f.hal).unwrap().current;
        let mut values = initial.clone();
        values.top_cover.as_mut().unwrap().brightness = 1;
        values.top_cover.as_mut().unwrap().color.b = 129;
        let settings = ProfileSettings {
            multi_zone_lighting: Some(fixture::target(&f)),
            lighting_options: Some(crate::lighting::options::Target {
                keyboard_type: 23,
                values,
            }),
            gpu: Some(crate::gpuctl::GpuClockSettings {
                core_offset_mhz: Some(60),
                mem_offset_mhz: None,
            }),
            ..Default::default()
        };
        let steps = apply_settings(&f.hal, &f.gpu, f.devices(), &settings).unwrap();
        let multi = steps
            .iter()
            .position(|step| step.step == "多区键盘灯效")
            .unwrap();
        let cover = steps
            .iter()
            .position(|step| step.step == "灯光策略与外围灯区")
            .unwrap();
        assert!(multi < cover);
        let data = f.device.0.lock().unwrap();
        let multi_send = data
            .events
            .iter()
            .position(|event| event.starts_with("MULTI:"))
            .unwrap();
        let cover_send = data
            .events
            .iter()
            .position(|event| event.starts_with("LIGHT:103:f4"))
            .unwrap();
        assert!(multi_send < cover_send);
        drop(data);
        if fail_gpu {
            let restore_multi = steps
                .iter()
                .position(|step| step.step == "恢复多区键盘灯效")
                .unwrap();
            let restore_cover = steps
                .iter()
                .position(|step| step.step == "恢复灯光策略与外围灯区")
                .unwrap();
            assert!(restore_multi < restore_cover);
            assert_eq!(f.lighting_options.read(&f.hal).unwrap().current, initial);
        } else {
            assert!(steps.iter().all(|step| step.ok));
            assert_eq!(
                f.lighting_options
                    .read(&f.hal)
                    .unwrap()
                    .current
                    .top_cover
                    .unwrap()
                    .brightness,
                1
            );
        }
    }
}

#[test]
fn multizone_toggle_keeps_colors_level_and_shared_initial_restore() {
    let mut f = Fixture::new();
    fixture::enable(&mut f);
    let before = f.multi_zone.read(&f.hal).unwrap().unwrap().current.unwrap();
    let identity = f.multi_zone.hotkey_identity(&f.hal).unwrap();
    assert!(!f.multi_zone.toggle_enabled(&f.hal, &identity).unwrap());
    let off = f.multi_zone.read(&f.hal).unwrap().unwrap().current.unwrap();
    assert_eq!(off.brightness, before.brightness);
    assert_eq!(off.zones, before.zones);
    assert_eq!(off.light_bar, before.light_bar);
    assert_eq!(off.cover, before.cover);
    assert!(f.multi_zone.toggle_enabled(&f.hal, &identity).unwrap());
    apply_settings(
        &f.hal,
        &f.gpu,
        f.devices(),
        &ProfileSettings {
            multi_zone_lighting: Some(fixture::target(&f)),
            ..Default::default()
        },
    )
    .unwrap();
    let current = f.multi_zone.read(&f.hal).unwrap().unwrap();
    f.multi_zone.restore(&f.hal, &current.revision).unwrap();
    assert_eq!(
        f.multi_zone.read(&f.hal).unwrap().unwrap().current.unwrap(),
        before
    );
}
#[test]
fn multizone_toggle_failure_recovers_and_capability_change_never_sends() {
    let mut f = Fixture::new();
    let io = fixture::enable(&mut f);
    let before = f.multi_zone.read(&f.hal).unwrap().unwrap().current.unwrap();
    let identity = f.multi_zone.hotkey_identity(&f.hal).unwrap();
    io.traffic.lock().unwrap().fail_sends.push(1);
    assert!(f.multi_zone.toggle_enabled(&f.hal, &identity).is_err());
    assert_eq!(
        f.multi_zone.read(&f.hal).unwrap().unwrap().current.unwrap(),
        before
    );
    f.device.0.lock().unwrap().bios[23] &= !32;
    let count = io.traffic.lock().unwrap().packets.len();
    assert!(f.multi_zone.toggle_enabled(&f.hal, &identity).is_err());
    assert_eq!(io.traffic.lock().unwrap().packets.len(), count);
}
