use super::*;
use crate::display::profile::Review;
#[path = "display_authority_tests.rs"]
mod authority;
#[path = "display_lifecycle_tests.rs"]
mod lifecycle;
use crate::profiles::application::{Application, Controller, Runtime};

struct Harness {
    f: Fixture,
    dir: tempfile::TempDir,
    application: Controller,
    file: crate::profiles::ProfileFile,
    review: Review,
}
impl Harness {
    fn new() -> Self {
        let f = Fixture::new();
        f.display.enable();
        let mut target = f.display.controller.capture().unwrap().unwrap();
        for mode in &mut target.displays {
            mode.frequency = 144;
        }
        let review = f.display.controller.preview_profile(&target, None).unwrap();
        let file = crate::profiles::ProfileFile {
            version: 1,
            name: "显示组合".into(),
            saved_at_ms: 1,
            settings: ProfileSettings {
                display: Some(target),
                pl1_w: Some(77.0),
                ..Default::default()
            },
        };
        let h = Self {
            f,
            dir: tempfile::tempdir().unwrap(),
            application: Controller::default(),
            file,
            review,
        };
        h.save();
        h
    }
    fn runtime(&self) -> Runtime<'_> {
        Runtime {
            origin: crate::profiles::application::Origin::Manual,
            permission: None,
            hal: &self.f.hal,
            gpu: &self.f.gpu,
            devices: self.f.devices(),
            dir: self.dir.path(),
        }
    }
    fn save(&self) {
        std::fs::write(
            self.dir.path().join("显示组合.json"),
            serde_json::to_vec(&self.file).unwrap(),
        )
        .unwrap();
    }
    fn enable_lan(&mut self) {
        use crate::platform::lan::{
            Target,
            model::{Field, Value},
        };
        self.f.lan.enable();
        *self.f.lan.io.gate.lock().unwrap() = Some(self.f.hal.control_gate());
        let mut target = Target::capture(&self.f.lan.io.frame.lock().unwrap())
            .unwrap()
            .unwrap();
        for device in &mut target.devices {
            device
                .values
                .insert(Field::PowerSavingMode, Some(Value::string("1")));
        }
        self.file.settings.lan_policy = Some(target);
        self.save();
    }
    fn begin(&self) -> Application {
        self.application
            .begin(
                self.runtime(),
                &self.file,
                None,
                Some(&self.review),
                None,
                None,
            )
            .unwrap()
    }
    fn no_other_writes(&self) {
        assert!(self.f.device.0.lock().unwrap().events.is_empty());
    }
    fn restored(&self) {
        assert!(
            self.f
                .display
                .io
                .0
                .lock()
                .unwrap()
                .displays
                .iter()
                .all(|display| display.current.frequency == 60)
        );
    }
}

#[test]
fn full_profile_waits_for_explicit_group_keep_before_any_other_output() {
    let h = Harness::new();
    let report = h.begin();
    assert!(report.steps.is_empty());
    assert_eq!(report.pending.unwrap().targets.len(), 2);
    h.no_other_writes();
    assert_eq!(h.f.display.io.0.lock().unwrap().writes.len(), 2);
    let report = h.application.keep(h.runtime(), report.id).unwrap();
    assert!(report.pending.is_none());
    assert!(report.steps.iter().all(|step| step.ok));
    assert!(report.steps.iter().any(|step| step.step == "保留档案显示"));
    assert_eq!(
        &h.f.device.0.lock().unwrap().pages[6][33..37],
        &77i32.to_le_bytes()
    );
    assert!(!h.f.display.controller.trial_pending().unwrap());
}

#[test]
fn cancellation_recovers_display_without_executing_other_targets() {
    let h = Harness::new();
    let id = h.begin().id;
    let report = h.application.cancel(h.runtime(), id).unwrap();
    assert!(report.pending.is_none());
    assert!(report.steps.iter().any(|step| !step.ok));
    h.no_other_writes();
    h.restored();
}

#[test]
fn timeout_records_incomplete_application_and_never_executes_remaining_settings() {
    let h = Harness::new();
    let id = h.begin().id;
    h.f.display.controller.tick(u64::MAX).unwrap();
    let report = h
        .application
        .read(&h.f.display.controller)
        .unwrap()
        .unwrap();
    assert!(report.pending.is_none());
    assert!(
        report
            .steps
            .iter()
            .any(|step| !step.ok && step.message.contains("未执行"))
    );
    assert!(h.application.keep(h.runtime(), id).is_err());
    h.no_other_writes();
    h.restored();
}

#[test]
fn file_change_during_trial_stops_other_outputs_and_recovers_display() {
    let mut h = Harness::new();
    let id = h.begin().id;
    h.file.settings.pl1_w = Some(78.0);
    h.save();
    let report = h.application.keep(h.runtime(), id).unwrap();
    assert!(
        report
            .steps
            .iter()
            .any(|step| !step.ok && step.message.contains("档案已变化"))
    );
    assert!(report.pending.is_none());
    h.no_other_writes();
    h.restored();
}

#[test]
fn every_target_is_preflighted_before_starting_any_display_trial() {
    let mut h = Harness::new();
    h.file.settings.pl1_w = Some(999.0);
    h.save();
    let report = h.begin();
    assert!(report.pending.is_none());
    assert!(report.steps.iter().any(|step| !step.ok));
    assert!(h.f.display.io.0.lock().unwrap().writes.is_empty());
    h.no_other_writes();
}

#[test]
fn gpu_failure_after_keep_restores_saved_cpu_and_this_application_display_before() {
    let mut h = Harness::new();
    h.file.settings.gpu = Some(crate::gpuctl::GpuClockSettings {
        core_offset_mhz: Some(60),
        mem_offset_mhz: None,
    });
    h.save();
    let before = h.f.device.0.lock().unwrap().pages[6];
    let id = h.begin().id;
    h.f.device.0.lock().unwrap().fail_gpu = true;
    let report = h.application.keep(h.runtime(), id).unwrap();
    assert!(
        report
            .steps
            .iter()
            .any(|step| step.step == "GPU 偏移" && !step.ok)
    );
    assert!(report.pending.is_none());
    assert_eq!(h.f.device.0.lock().unwrap().pages[6], before);
    h.restored();
}

#[test]
fn failed_display_compensation_retains_session_and_allows_explicit_retry() {
    let mut h = Harness::new();
    let id = h.begin().id;
    h.file.settings.pl1_w = Some(78.0);
    h.save();
    h.f.display.io.0.lock().unwrap().fail_at = vec![3];
    let report = h.application.keep(h.runtime(), id).unwrap();
    assert!(report.pending.is_some());
    assert!(
        report
            .steps
            .iter()
            .any(|step| step.step == "恢复本次显示变更" && !step.ok)
    );
    assert!(h.application.keep(h.runtime(), id).is_err());
    h.f.display.io.0.lock().unwrap().fail_at.clear();
    let report = h.application.cancel(h.runtime(), id).unwrap();
    assert!(report.pending.is_none());
    h.no_other_writes();
    h.restored();
}

#[test]
fn no_change_display_applies_other_settings_without_guard_or_keep() {
    let mut h = Harness::new();
    h.file.settings.display = h.f.display.controller.capture().unwrap();
    h.review =
        h.f.display
            .controller
            .preview_profile(h.file.settings.display.as_ref().unwrap(), None)
            .unwrap();
    h.save();
    let report = h.begin();
    assert!(report.pending.is_none());
    assert!(report.steps.iter().all(|step| step.ok));
    assert!(h.f.display.io.0.lock().unwrap().writes.is_empty());
}

#[test]
fn missing_or_stale_display_preview_prevents_all_outputs() {
    let h = Harness::new();
    let report = h
        .application
        .begin(h.runtime(), &h.file, None, None, None, None)
        .unwrap();
    assert!(report.steps.iter().any(|step| !step.ok));
    h.f.display.io.0.lock().unwrap().displays[1]
        .current
        .frequency = 144;
    let report = h.begin();
    assert!(report.steps.iter().any(|step| !step.ok));
    assert!(h.f.display.io.0.lock().unwrap().writes.is_empty());
    h.no_other_writes();
}

#[test]
fn competing_application_and_wrong_confirmation_do_not_consume_current_trial() {
    let h = Harness::new();
    let id = h.begin().id;
    assert!(
        h.application
            .begin(h.runtime(), &h.file, None, Some(&h.review), None, None)
            .is_err()
    );
    assert!(h.application.keep(h.runtime(), id + 1).is_err());
    assert!(h.application.cancel(h.runtime(), id + 1).is_err());
    assert_eq!(
        h.application
            .read(&h.f.display.controller)
            .unwrap()
            .unwrap()
            .id,
        id
    );
    assert_eq!(h.f.display.io.0.lock().unwrap().writes.len(), 2);
    h.no_other_writes();
    h.application.cancel(h.runtime(), id).unwrap();
}

#[test]
fn retained_external_change_stops_remainder_without_overwriting_external_mode() {
    let h = Harness::new();
    let id = h.begin().id;
    h.f.display.io.0.lock().unwrap().displays[1]
        .current
        .frequency = 120;
    let report = h.application.keep(h.runtime(), id).unwrap();
    assert!(report.steps.iter().any(|step| !step.ok));
    assert!(report.pending.is_some());
    assert_eq!(
        h.f.display.io.0.lock().unwrap().displays[1]
            .current
            .frequency,
        120
    );
    h.no_other_writes();
    h.f.display.io.0.lock().unwrap().displays[1]
        .current
        .frequency = 144;
    h.application.cancel(h.runtime(), id).unwrap();
    h.restored();
}

#[test]
fn raw_transaction_cannot_bypass_display_confirmation() {
    let h = Harness::new();
    assert!(apply_settings(&h.f.hal, &h.f.gpu, h.f.devices(), &h.file.settings).is_err());
    assert!(h.f.display.io.0.lock().unwrap().writes.is_empty());
    h.no_other_writes();
}

#[test]
fn touchpad_waits_for_display_keep_and_cancel_never_toggles() {
    let mut h = Harness::new();
    h.f.touchpad_io.enable();
    h.file.settings.touchpad = Some(crate::touchpad::Target {
        devices: h.f.touchpad_io.devices.lock().unwrap().clone(),
        enabled: false,
        saved_enabled: true,
    });
    h.save();
    let id = h.begin().id;
    assert!(h.f.touchpad_io.writes.lock().unwrap().is_empty());
    h.application.cancel(h.runtime(), id).unwrap();
    assert!(h.f.touchpad_io.writes.lock().unwrap().is_empty());
    let id = h.begin().id;
    let result = h.application.keep(h.runtime(), id).unwrap();
    assert!(result.targets_applied);
    let p = h.f.touchpad.read(&h.f.hal).unwrap();
    assert_eq!(p.current.enabled, Some(false));
    assert_eq!(p.current.saved, 1);
}

#[test]
fn replaced_touchpad_during_display_trial_prevents_cpu_and_input_writes() {
    let mut h = Harness::new();
    h.f.touchpad_io.enable();
    h.file.settings.touchpad = Some(crate::touchpad::Target {
        devices: h.f.touchpad_io.devices.lock().unwrap().clone(),
        enabled: false,
        saved_enabled: true,
    });
    h.save();
    let id = h.begin().id;
    *h.f.touchpad_io.devices.lock().unwrap() = vec!["replacement".into()];
    let result = h.application.keep(h.runtime(), id).unwrap();
    assert!(!result.targets_applied && result.steps.iter().any(|s| !s.ok));
    assert!(h.f.touchpad_io.writes.lock().unwrap().is_empty());
    h.no_other_writes();
    h.restored();
}
