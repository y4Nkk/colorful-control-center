use super::fixture::Harness;
use crate::{
    hal::power_events::{Event, Source},
    profiles::application::Origin,
};
use ccc_core::{
    fan::{CurvePoint, FanCurve},
    software_fan::Curves,
};

fn curves() -> Curves {
    [(
        "cpu".into(),
        FanCurve {
            points: vec![
                CurvePoint {
                    temp_c: 40.0,
                    duty_pct: 30.0,
                },
                CurvePoint {
                    temp_c: 95.0,
                    duty_pct: 100.0,
                },
            ],
            hysteresis_c: 3.0,
            ramp_pct_per_s: 10.0,
        },
    )]
    .into()
}

#[test]
fn restarted_startup_and_automatic_applications_preserve_unclean_faulted_and_invalid_journals() {
    use crate::profiles::application::{Permission, Runtime};
    for bytes in [
        br#"{"version":1,"active":true,"faulted":false}"#.as_slice(),
        br#"{"version":1,"active":false,"faulted":true}"#,
        b"invalid-journal",
    ] {
        let mut h = Harness::new(false);
        let path = h.dir.path().join(crate::hal::SOFTWARE_FAN_RECORD_NAME);
        std::fs::write(&path, bytes).unwrap();
        h.f.hal.bind_software_fan_record(path.clone());
        let mut file = h.file("电池");
        file.settings.fan_software_curves = Some(curves());
        h.write(&file);
        let dir = h.profiles();
        let before = h.f.device.0.lock().unwrap().events.clone();
        for origin in [Origin::Startup, Origin::Ac, Origin::Battery, Origin::Resume] {
            let report = h
                .application
                .begin(
                    Runtime {
                        origin,
                        permission: Some(Permission::new(|| Ok(()))),
                        dir: &dir,
                        ..h.runtime()
                    },
                    &file,
                    None,
                    None,
                    None,
                    None,
                )
                .unwrap();
            assert!(!report.targets_applied);
            assert!(report.pending.is_none());
            assert!(
                report
                    .steps
                    .iter()
                    .any(|step| !step.ok && step.message.contains("因故障停止"))
            );
            assert_eq!(std::fs::read(&path).unwrap(), bytes);
            assert_eq!(h.f.device.0.lock().unwrap().events, before);
        }
    }
}

#[test]
fn supply_and_resume_cannot_restart_a_faulted_curve_or_begin_its_display_trial() {
    let h = Harness::new(true);
    for name in ["插电", "电池", "唤醒"] {
        let mut file = h.file(name);
        file.settings.fan_software_curves = Some(curves());
        h.write(&file);
    }
    h.enable(true);
    h.f.hal.latch_software_fan_fault();
    let before = h.f.device.0.lock().unwrap().events.clone();
    let display = h.f.display.controller.capture().unwrap();
    for (events, now, origin) in [
        (vec![Event::Source(Source::Battery)], 1, Origin::Battery),
        (vec![Event::Source(Source::Ac)], 2_000, Origin::Ac),
        (vec![Event::Suspend, Event::Resume], 4_000, Origin::Resume),
    ] {
        for event in events {
            h.observe(event, now);
        }
        h.tick(now + 1_000).unwrap();
        let report = h.last().unwrap();
        assert_eq!(report.origin, origin);
        assert!(!report.targets_applied);
        assert!(report.pending.is_none());
        assert!(
            report
                .steps
                .iter()
                .any(|step| !step.ok && step.message.contains("因故障停止"))
        );
        assert_eq!(h.f.device.0.lock().unwrap().events, before);
        assert!(h.f.display.controller.capture().unwrap() == display);
        assert!(h.f.hal.ensure_software_fan_restart_allowed().is_err());
    }
}

#[test]
fn fan_fault_does_not_block_unrelated_automatic_targets_or_clear_on_firmware_restore() {
    let h = Harness::new(false);
    h.enable(false);
    h.f.hal.latch_software_fan_fault();
    h.observe(Event::Source(Source::Battery), 1);
    h.tick(1_001).unwrap();
    assert!(h.last().unwrap().targets_applied);
    assert_eq!(h.watts(), 55);
    h.f.hal.disarm_fan_guard().unwrap();
    assert!(h.f.hal.acknowledge_software_fan_restart().is_err());
    assert!(h.f.hal.ensure_software_fan_restart_allowed().is_err());
}
