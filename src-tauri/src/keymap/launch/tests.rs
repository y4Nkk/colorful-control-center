use super::*;
use ccc_core::keymap::{Binding, Configuration, DeviceSettings};
fn record() -> Record {
    let configuration = Configuration {
        devices: [Device::Keyboard, Device::Mouse]
            .map(|device| DeviceSettings {
                device,
                enabled: true,
                bindings: vec![Binding {
                    trigger: 4,
                    action: Action::Launch {
                        path: format!("{device:?}.exe"),
                        event_id: if device == Device::Keyboard { 256 } else { 257 },
                    },
                }],
            })
            .to_vec(),
    };
    Record {
        version: 1,
        baseline: super::super::model::Baseline {
            name: "fixture".into(),
            configuration: configuration.clone(),
            flags: [1, 1],
        },
        current: configuration,
        owned: vec![Device::Keyboard],
        phase: Phase::Applied,
    }
}
fn event(source: &str, id: u32) -> event_log::Record {
    event_log::Record {
        number: 1,
        event_id: id,
        event_type: 4,
        source: source.into(),
        strings: vec![],
    }
}
#[test]
fn matches_exact_driver_source_severity_device_and_owned_enabled_event_only() {
    let mut record = record();
    let configured = programs(&record);
    assert_eq!(
        select(&configured, &event("HKKbdFltr", 0x40000100)),
        Some("Keyboard.exe")
    );
    for event in [
        event("other", 0x40000100),
        event("HKMouFltr", 0x40000100),
        event("HKKbdFltr", 0xc0000100),
        event("HKKbdFltr", 0x40000101),
    ] {
        assert!(select(&configured, &event).is_none());
    }
    record.owned.push(Device::Mouse);
    assert_eq!(
        select(&programs(&record), &event("hkmoufltr", 0x40000101)),
        Some("Mouse.exe")
    );
    record.current.devices[0].enabled = false;
    assert_eq!(programs(&record).len(), 1);
}
#[test]
fn altered_missing_or_prepared_records_never_authorize_a_program() {
    let expected = record();
    assert!(authorize(Some(&expected), &expected).is_ok());
    assert!(authorize(None, &expected).is_err());
    let mut altered = expected.clone();
    altered.current.devices[0].bindings.clear();
    assert!(authorize(Some(&altered), &expected).is_err());
    altered = expected.clone();
    altered.phase = Phase::Prepared;
    assert!(authorize(Some(&altered), &altered).is_err());
    let controller = Controller::default();
    assert!(!controller.status().unwrap().running);
    controller.replace(None).unwrap();
}
#[test]
fn recording_prepared_foreign_and_changed_events_never_reach_the_launcher() {
    let expected = record();
    let programs = programs(&expected);
    let valid = event("HKKbdFltr", 0x40000100);
    fn never(_: &str) -> Result<()> {
        panic!("不应调用程序执行器")
    }
    assert!(
        !dispatch(
            &event("other", 0x40000100),
            &programs,
            &expected,
            || panic!("不应检查录制"),
            || panic!("不应读取记录"),
            never
        )
        .unwrap()
    );
    assert!(
        !dispatch(
            &valid,
            &programs,
            &expected,
            || Ok(true),
            || panic!("录制时不读取记录"),
            never
        )
        .unwrap()
    );
    let mut preparing = expected.clone();
    preparing.phase = Phase::Prepared;
    assert!(
        !dispatch(
            &valid,
            &programs,
            &expected,
            || Ok(false),
            || Ok(Some(preparing)),
            never
        )
        .unwrap()
    );
    assert!(
        dispatch(
            &valid,
            &programs,
            &expected,
            || Ok(false),
            || Ok(None),
            never
        )
        .is_err()
    );
}
#[test]
fn confirmed_events_call_the_injected_launcher_and_preserve_its_error() {
    let expected = record();
    let programs = programs(&expected);
    let valid = event("HKKbdFltr", 0x40000100);
    let mut launched = Vec::new();
    assert!(
        dispatch(
            &valid,
            &programs,
            &expected,
            || Ok(false),
            || Ok(Some(expected.clone())),
            |path| {
                launched.push(path.to_owned());
                Ok(())
            }
        )
        .unwrap()
    );
    assert_eq!(launched, ["Keyboard.exe"]);
    assert!(
        dispatch(
            &valid,
            &programs,
            &expected,
            || Ok(false),
            || Ok(Some(expected.clone())),
            |_| bail!("隔离启动失败")
        )
        .unwrap_err()
        .to_string()
        .contains("隔离启动失败")
    );
}
