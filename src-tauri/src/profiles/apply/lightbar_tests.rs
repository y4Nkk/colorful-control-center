use super::*;
use crate::lighting::lightbar::{self, Kind};
#[path = "lightbar_fixture.rs"]
mod memory;
use memory::{fixture, target};

#[test]
fn capture_and_disk_roundtrip_preserve_both_lightbar_contracts_without_writing() {
    for kind in [Kind::X170, Kind::X58] {
        let (f, io) = fixture(kind);
        let captured = crate::profiles::capture_current(&f.hal, &f.gpu, f.devices(), &crate::profiles::ProfileGroup::ALL).unwrap();
        assert_eq!(captured.light_bar, Some(io.snapshot().values().unwrap()));
        let dir = tempfile::tempdir().unwrap();
        crate::profiles::save_in(dir.path(), captured, "完整灯条", None).unwrap();
        let file = crate::profiles::load_from(dir.path(), "完整灯条").unwrap();
        assert_eq!(
            file.settings.light_bar,
            Some(io.snapshot().values().unwrap())
        );
        assert!(f.device.0.lock().unwrap().events.is_empty());
        assert!(io.0.lock().unwrap().sends.is_empty());
    }
}

#[test]
fn strict_lightbar_only_profile_accepts_current_tags_and_rejects_unknown_or_invalid_targets() {
    for kind in [Kind::X170, Kind::X58] {
        let value = serde_json::json!({"lightBar": target(kind)});
        let settings: ProfileSettings = serde_json::from_value(value.clone()).unwrap();
        crate::profiles::validate(&settings).unwrap();
        let mut invalid = value.clone();
        invalid["lightBar"]["settings"]["brightness"] = 5.into();
        assert!(crate::profiles::validate(&serde_json::from_value(invalid).unwrap()).is_err());
        let mut unknown = value.clone();
        unknown["lightBar"]["settings"]["legacy"] = true.into();
        assert!(serde_json::from_value::<ProfileSettings>(unknown).is_err());
        let mut wrong = value;
        wrong["lightBar"]["kind"] = "unknown".into();
        assert!(serde_json::from_value::<ProfileSettings>(wrong).is_err());
    }
}

#[test]
fn complete_preflight_rejects_wrong_device_or_owner_before_any_cpu_or_gpu_write() {
    let (f, io) = fixture(Kind::X170);
    let mut settings = settings();
    settings.light_bar = Some(target(Kind::X58));
    assert!(apply_settings(&f.hal, &f.gpu, f.devices(), &settings).is_err());
    settings.light_bar = Some(target(Kind::X170));
    io.0.lock().unwrap().unavailable = true;
    assert!(apply_settings(&f.hal, &f.gpu, f.devices(), &settings).is_err());
    assert!(f.device.0.lock().unwrap().events.is_empty());
    assert!(io.0.lock().unwrap().sends.is_empty());
}

#[test]
fn declared_test_device_without_injected_transport_never_falls_back_to_native_hid() {
    let f = Fixture::new();
    {
        let mut state = f.device.0.lock().unwrap();
        state.bios[0] = 1;
        state.bios[11] = 8;
    }
    let settings = ProfileSettings {
        light_bar: Some(target(Kind::X170)),
        ..settings()
    };
    let error = apply_settings(&f.hal, &f.gpu, f.devices(), &settings)
        .err()
        .unwrap();
    assert!(format!("{error:#}").contains("未访问真实HID"));
    assert!(f.device.0.lock().unwrap().events.is_empty());
}

#[test]
fn later_gpu_failure_restores_immediate_and_saved_lightbar_and_cpu_for_both_kinds() {
    for kind in [Kind::X170, Kind::X58] {
        let (f, io) = fixture(kind);
        let initial = io.snapshot();
        let original_power = f.device.0.lock().unwrap().power;
        f.device.0.lock().unwrap().fail_gpu = true;
        let settings = ProfileSettings {
            light_bar: Some(target(kind)),
            gpu: Some(crate::gpuctl::GpuClockSettings {
                core_offset_mhz: Some(60),
                mem_offset_mhz: None,
            }),
            ..settings()
        };
        let steps = apply_settings(&f.hal, &f.gpu, f.devices(), &settings).unwrap();
        for (name, ok) in [
            ("即时 CPU 调校", true),
            ("独立灯条", true),
            ("GPU 偏移", false),
            ("恢复独立灯条", true),
            ("恢复即时 CPU", true),
        ] {
            assert!(steps.iter().any(|step| step.step == name && step.ok == ok));
        }
        io.assert_restored(&initial);
        assert_eq!(f.device.0.lock().unwrap().power, original_power);
        assert_eq!(&f.device.0.lock().unwrap().voltage[..2], &[-51, -26]);
    }
}

#[test]
fn partial_save_self_compensates_once_and_stops_later_gpu_step() {
    let (f, io) = fixture(Kind::X58);
    let initial = io.snapshot();
    io.0.lock().unwrap().fail_saves.insert(1);
    let settings = ProfileSettings {
        light_bar: Some(target(Kind::X58)),
        gpu: Some(crate::gpuctl::GpuClockSettings {
            core_offset_mhz: Some(60),
            mem_offset_mhz: None,
        }),
        ..settings()
    };
    let steps = apply_settings(&f.hal, &f.gpu, f.devices(), &settings).unwrap();
    assert!(
        steps
            .iter()
            .any(|step| step.step == "独立灯条" && !step.ok && step.message.contains("已恢复"))
    );
    assert!(
        !steps
            .iter()
            .any(|step| step.step == "GPU 偏移" || step.step == "恢复独立灯条")
    );
    io.assert_restored(&initial);
    assert_eq!(io.0.lock().unwrap().saves, 2);
}

#[test]
fn failed_local_compensation_is_retried_by_outer_profile_and_user_baseline_survives() {
    let (f, io) = fixture(Kind::X170);
    let initial = io.snapshot();
    io.0.lock().unwrap().fail_saves.extend([1, 2]);
    let settings = ProfileSettings {
        light_bar: Some(target(Kind::X170)),
        ..Default::default()
    };
    let steps = apply_settings(&f.hal, &f.gpu, f.devices(), &settings).unwrap();
    assert!(
        steps
            .iter()
            .any(|step| step.step == "独立灯条" && !step.ok && step.message.contains("恢复失败"))
    );
    assert!(
        steps
            .iter()
            .any(|step| step.step == "恢复独立灯条" && step.ok)
    );
    assert_eq!(io.0.lock().unwrap().saves, 3);
    io.assert_restored(&initial);
    apply_settings(&f.hal, &f.gpu, f.devices(), &settings).unwrap();
    let revision = f.light_bar.read(&f.hal).unwrap().unwrap().revision;
    f.light_bar.restore(&f.hal, &revision).unwrap();
    io.assert_restored(&initial);
}

#[test]
fn single_and_profile_edits_share_first_baseline_and_recovery_changes_only_owned_bytes() {
    let (f, io) = fixture(Kind::X58);
    let initial = io.snapshot();
    let current = f.light_bar.read(&f.hal).unwrap().unwrap();
    f.light_bar
        .apply(
            &f.hal,
            &lightbar::Request {
                expected_revision: current.revision,
                values: target(Kind::X58),
            },
        )
        .unwrap();
    let mut next = target(Kind::X58);
    if let lightbar::Values::X58(value) = &mut next {
        value.brightness = 4;
    }
    let settings = ProfileSettings {
        light_bar: Some(next),
        ..Default::default()
    };
    assert!(
        apply_settings(&f.hal, &f.gpu, f.devices(), &settings)
            .unwrap()
            .iter()
            .all(|step| step.ok)
    );
    let current = f.light_bar.read(&f.hal).unwrap().unwrap();
    assert_eq!(current.initial, Some(initial.values().unwrap()));
    assert_eq!(&io.snapshot().bytes[5..], &initial.bytes[5..]);
    f.light_bar.restore(&f.hal, &current.revision).unwrap();
    io.assert_restored(&initial);
}

#[test]
fn changed_profile_lightbar_target_rejects_old_confirmation_before_any_write() {
    let (f, io) = fixture(Kind::X58);
    let reviewed = crate::profiles::ProfileFile {
        version: 1,
        name: "灯条".into(),
        saved_at_ms: 1,
        settings: ProfileSettings {
            light_bar: Some(target(Kind::X58)),
            ..settings()
        },
    };
    let mut actual = reviewed.clone();
    if let Some(lightbar::Values::X58(value)) = &mut actual.settings.light_bar {
        value.brightness = 4;
    }
    assert!(
        apply_reviewed(
            &f.hal,
            &f.gpu,
            f.devices(),
            &actual,
            &reviewed,
            None,
            None,
            None
        )
        .is_err()
    );
    assert!(f.device.0.lock().unwrap().events.is_empty());
    assert!(io.0.lock().unwrap().sends.is_empty());
}

#[test]
fn profile_and_single_operations_serialize_without_relocking_the_global_gate() {
    let (f, io) = fixture(Kind::X58);
    let initial = io.snapshot();
    let start = std::sync::Barrier::new(2);
    std::thread::scope(|threads| {
        threads.spawn(|| {
            start.wait();
            for _ in 0..20 {
                let settings = ProfileSettings {
                    light_bar: Some(target(Kind::X58)),
                    ..Default::default()
                };
                assert!(
                    apply_settings(&f.hal, &f.gpu, f.devices(), &settings)
                        .unwrap()
                        .iter()
                        .all(|step| step.ok)
                );
            }
        });
        threads.spawn(|| {
            start.wait();
            for _ in 0..20 {
                let current = f.light_bar.read(&f.hal).unwrap().unwrap();
                let outcome = f.light_bar.apply(
                    &f.hal,
                    &lightbar::Request {
                        expected_revision: current.revision,
                        values: initial.values().unwrap(),
                    },
                );
                if let Err(error) = outcome {
                    assert!(error.to_string().contains("预览已变化"));
                }
                assert!(
                    crate::profiles::capture_current(&f.hal, &f.gpu, f.devices(), &crate::profiles::ProfileGroup::ALL)
                        .unwrap()
                        .light_bar
                        .is_some()
                );
            }
        });
    });
    let current = f.light_bar.read(&f.hal).unwrap().unwrap();
    f.light_bar.restore(&f.hal, &current.revision).unwrap();
    io.assert_restored(&initial);
}

#[test]
fn combined_cpu_dust_lightbar_and_gpu_failure_restores_all_domains_then_retries() {
    let (f, io) = fixture(Kind::X58);
    let light_initial = io.snapshot();
    let dust_initial = [1, 193, 23, 77, 88];
    {
        let mut state = f.device.0.lock().unwrap();
        state.bios[0] = 1;
        state.bios[10] = 128;
        state.pages[4][81..86].copy_from_slice(&dust_initial);
        state.fail_gpu = true;
    }
    let settings = ProfileSettings {
        light_bar: Some(target(Kind::X58)),
        dust_schedule: Some(fanctl::dust::Schedule {
            weekdays: 0,
            hour: 0,
            on_resume: false,
        }),
        gpu: Some(crate::gpuctl::GpuClockSettings {
            core_offset_mhz: Some(60),
            mem_offset_mhz: None,
        }),
        ..settings()
    };
    let steps = apply_settings(&f.hal, &f.gpu, f.devices(), &settings).unwrap();
    for name in [
        "除尘计划",
        "即时 CPU 调校",
        "独立灯条",
        "恢复独立灯条",
        "恢复即时 CPU",
        "恢复除尘计划",
    ] {
        assert!(steps.iter().any(|s| s.step == name && s.ok));
    }
    assert!(steps.iter().any(|s| s.step == "GPU 偏移" && !s.ok));
    io.assert_restored(&light_initial);
    assert_eq!(&f.device.0.lock().unwrap().pages[4][81..86], &dust_initial);
    f.device.0.lock().unwrap().fail_gpu = false;
    assert!(
        apply_settings(&f.hal, &f.gpu, f.devices(), &settings)
            .unwrap()
            .iter()
            .all(|s| s.ok)
    );
    let light = f.light_bar.read(&f.hal).unwrap().unwrap();
    assert_eq!(light.initial, Some(light_initial.values().unwrap()));
    f.light_bar.restore(&f.hal, &light.revision).unwrap();
    let dust = serde_json::to_value(f.dust.read(&f.hal).unwrap().unwrap()).unwrap();
    f.dust
        .restore(&f.hal, dust["revision"].as_str().unwrap())
        .unwrap();
    io.assert_restored(&light_initial);
    assert_eq!(&f.device.0.lock().unwrap().pages[4][81..86], &dust_initial);
}
