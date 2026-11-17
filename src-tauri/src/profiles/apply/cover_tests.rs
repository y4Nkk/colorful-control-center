use super::perkey_tests::fixture::Harness;
use super::*;
use crate::lighting::options::{self, TopCover};
use crate::profiles::{
    ProfileFile,
    application::{Controller, Runtime},
};

fn setup() -> Harness {
    let h = Harness::new();
    {
        let mut data = h.f.device.0.lock().unwrap();
        data.bios[23] |= 64;
        data.pages[2][100..104].copy_from_slice(&[3, 10, 20, 30]);
    }
    h
}
fn target(h: &Harness, brightness: u8) -> options::Target {
    let scope = h.f.lighting_options.scope().unwrap();
    let _gate = h.f.hal.begin_write().unwrap();
    let mut target = scope.capture(&h.f.hal).unwrap().unwrap();
    target.values.top_cover.as_mut().unwrap().brightness = brightness;
    target.values.top_cover.as_mut().unwrap().color.b = 129;
    target
}
fn runtime<'a>(h: &'a Harness, dir: &'a std::path::Path, startup: bool) -> Runtime<'a> {
    Runtime {
        origin: if startup {
            crate::profiles::application::Origin::Startup
        } else {
            crate::profiles::application::Origin::Manual
        },
        permission: startup.then(|| crate::profiles::application::Permission::new(|| Ok(()))),
        hal: &h.f.hal,
        gpu: &h.f.gpu,
        devices: h.f.devices(),
        dir,
    }
}

#[test]
fn complete_capture_roundtrip_includes_independent_cover_and_perkey() {
    let h = setup();
    let settings = crate::profiles::capture_current(&h.f.hal, &h.f.gpu, h.f.devices()).unwrap();
    assert!(settings.perkey_lighting.is_some());
    assert!(settings.keyboard_lighting.is_none());
    let cover = settings
        .lighting_options
        .as_ref()
        .unwrap()
        .values
        .top_cover
        .as_ref()
        .unwrap();
    assert_eq!(cover.brightness, 3);
    assert_eq!(cover.color.b, 30);
    assert!(
        serde_json::from_str::<ProfileSettings>(&serde_json::to_string(&settings).unwrap())
            .unwrap()
            == settings
    );
    assert!(h.f.device.0.lock().unwrap().events.is_empty());
}
#[test]
fn later_gpu_failure_restores_cover_after_keyboard_and_preserves_unowned_fields() {
    let h = setup();
    let before = h.f.lighting_options.read(&h.f.hal).unwrap().current;
    let mut settings = ProfileSettings {
        lighting_options: Some(target(&h, 1)),
        perkey_lighting: Some(h.target(false)),
        ..Default::default()
    };
    super::perkey_tests::fixture::failing_gpu(&mut settings);
    h.f.device.0.lock().unwrap().fail_gpu = true;
    let steps = apply_settings(&h.f.hal, &h.f.gpu, h.f.devices(), &settings).unwrap();
    assert!(steps.iter().any(|step| !step.ok && step.step == "GPU 偏移"));
    let perkey = steps
        .iter()
        .position(|step| step.step == "恢复逐键灯效")
        .unwrap();
    let cover = steps
        .iter()
        .position(|step| step.step == "恢复灯光策略与外围灯区")
        .unwrap();
    assert!(perkey < cover);
    assert_eq!(h.f.lighting_options.read(&h.f.hal).unwrap().current, before);
}
#[test]
fn profile_failure_restores_current_cover_while_initial_restore_keeps_first_value() {
    let h = setup();
    let profile = h.f.lighting_options.read(&h.f.hal).unwrap();
    h.f.lighting_options
        .apply(
            &h.f.hal,
            &options::Request {
                expected_revision: profile.revision,
                values: target(&h, 1).values,
            },
        )
        .unwrap();
    let first = h.f.lighting_options.read(&h.f.hal).unwrap();
    assert_eq!(
        first
            .initial
            .as_ref()
            .unwrap()
            .top_cover
            .as_ref()
            .unwrap()
            .brightness,
        3
    );
    let mut settings = ProfileSettings {
        lighting_options: Some(target(&h, 2)),
        ..Default::default()
    };
    super::perkey_tests::fixture::failing_gpu(&mut settings);
    h.f.device.0.lock().unwrap().fail_gpu = true;
    let steps = apply_settings(&h.f.hal, &h.f.gpu, h.f.devices(), &settings).unwrap();
    assert!(steps.iter().any(|step| !step.ok));
    let profile = h.f.lighting_options.read(&h.f.hal).unwrap();
    assert_eq!(profile.current.top_cover.as_ref().unwrap().brightness, 1);
    h.f.lighting_options
        .restore(&h.f.hal, &profile.revision)
        .unwrap();
    assert_eq!(
        h.f.lighting_options
            .read(&h.f.hal)
            .unwrap()
            .current
            .top_cover
            .as_ref()
            .unwrap()
            .brightness,
        3
    );
}
#[test]
fn capability_change_blocks_cover_and_all_other_targets() {
    let h = setup();
    let settings = ProfileSettings {
        lighting_options: Some(target(&h, 2)),
        pl1_w: Some(77.0),
        ..Default::default()
    };
    h.f.device.0.lock().unwrap().bios[23] &= !64;
    assert!(apply_settings(&h.f.hal, &h.f.gpu, h.f.devices(), &settings).is_err());
    assert!(h.f.device.0.lock().unwrap().events.is_empty());
}
#[test]
fn manual_and_startup_use_the_same_cover_file_target() {
    for startup in [false, true] {
        let h = setup();
        let dir = tempfile::tempdir().unwrap();
        let file = ProfileFile {
            version: 1,
            name: "独立顶盖".into(),
            saved_at_ms: 1,
            settings: ProfileSettings {
                lighting_options: Some(target(&h, 2)),
                ..Default::default()
            },
        };
        std::fs::write(
            dir.path().join("独立顶盖.json"),
            serde_json::to_vec(&file).unwrap(),
        )
        .unwrap();
        let report = Controller::default()
            .begin(
                runtime(&h, dir.path(), startup),
                &file,
                None,
                None,
                None,
                None,
            )
            .unwrap();
        assert!(
            report.targets_applied
                && report.origin
                    == if startup {
                        crate::profiles::application::Origin::Startup
                    } else {
                        crate::profiles::application::Origin::Manual
                    }
        );
        assert_eq!(
            h.f.lighting_options
                .read(&h.f.hal)
                .unwrap()
                .current
                .top_cover
                .as_ref()
                .unwrap()
                .brightness,
            2
        );
    }
}
#[test]
fn top_cover_json_contract_rejects_brightness_and_unknown_fields() {
    assert!(
        serde_json::from_str::<TopCover>(r#"{"brightness":256,"color":{"r":0,"g":0,"b":0}}"#)
            .is_err()
    );
    let h = setup();
    let mut wanted = target(&h, 5);
    assert!(wanted.validate().is_err());
    wanted.values.top_cover.as_mut().unwrap().brightness = 2;
    let mut raw = serde_json::to_value(&wanted).unwrap();
    raw["values"]["topCover"]["legacyEnabled"] = true.into();
    assert!(serde_json::from_value::<options::Target>(raw).is_err());
}
