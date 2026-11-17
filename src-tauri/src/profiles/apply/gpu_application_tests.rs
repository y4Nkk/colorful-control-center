use super::*;
use crate::profiles::{
    ProfileFile,
    application::{Application, Controller, Runtime},
};

struct Harness {
    f: Fixture,
    dir: tempfile::TempDir,
    application: Controller,
    file: ProfileFile,
    review: crate::gpu_power::profile::Review,
}
impl Harness {
    fn new() -> Self {
        let f = setup(false);
        let mut settings = wanted(&f, true);
        settings.pl1_w = Some(77.0);
        settings.gpu = Some(crate::gpuctl::GpuClockSettings {
            core_offset_mhz: Some(60),
            mem_offset_mhz: None,
        });
        let review = {
            let scope = f.gpu_power.scope().unwrap();
            let _gate = f.hal.begin_write().unwrap();
            scope
                .preview_profile(&f.hal, settings.gpu_power.as_ref().unwrap())
                .unwrap()
        };
        let h = Self {
            f,
            dir: tempfile::tempdir().unwrap(),
            application: Controller::default(),
            file: ProfileFile {
                version: 1,
                name: "独显组合".into(),
                saved_at_ms: 1,
                settings,
            },
            review,
        };
        h.save();
        h
    }
    fn save(&self) {
        std::fs::write(
            self.dir.path().join("独显组合.json"),
            serde_json::to_vec(&self.file).unwrap(),
        )
        .unwrap();
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
    fn begin(&self, display: Option<&crate::display::profile::Review>) -> Application {
        self.application
            .begin(
                self.runtime(),
                &self.file,
                None,
                display,
                Some(&self.review),
                None,
            )
            .unwrap()
    }
    fn display(&mut self) -> crate::display::profile::Review {
        self.f.display.enable();
        let mut target = self.f.display.controller.capture().unwrap().unwrap();
        for mode in &mut target.displays {
            mode.frequency = 144;
        }
        let review = self
            .f
            .display
            .controller
            .preview_profile(&target, None)
            .unwrap();
        self.file.settings.display = Some(target);
        self.save();
        review
    }
}
#[test]
fn manual_application_requires_complete_gpu_review_before_authorization() {
    let h = Harness::new();
    let result = h
        .application
        .begin(h.runtime(), &h.file, None, None, None, None)
        .unwrap();
    assert!(!result.targets_applied);
    assert!(
        result
            .steps
            .iter()
            .any(|s| !s.ok && s.message.contains("独显预览"))
    );
    assert!(h.f.gpu_environment.io.0.lock().unwrap().events.is_empty());
    assert_eq!(h.f.gpu_environment.authorizations.load(Ordering::SeqCst), 0);
}
#[test]
fn stale_gpu_preview_and_file_changed_during_authorization_have_zero_outputs() {
    for changed_file in [false, true] {
        let h = Harness::new();
        if changed_file {
            let mut file = h.file.clone();
            file.settings.pl1_w = Some(78.0);
            let path = h.dir.path().join("独显组合.json");
            h.f.gpu_environment.io.0.lock().unwrap().authorize_hook = Some(Box::new(move || {
                std::fs::write(path, serde_json::to_vec(&file).unwrap()).unwrap();
            }));
        } else {
            h.f.gpu_environment.io.0.lock().unwrap().device.name = "changed".into();
        }
        let report = h.begin(None);
        assert!(!report.targets_applied);
        assert!(h.f.gpu_environment.io.0.lock().unwrap().events.is_empty());
        assert!(h.f.device.0.lock().unwrap().events.is_empty());
    }
}
#[test]
fn display_trial_precedes_gpu_authorization_and_keep_continues_full_profile() {
    let mut h = Harness::new();
    let display = h.display();
    let report = h.begin(Some(&display));
    assert!(report.pending.is_some());
    assert!(!h.f.gpu_environment.io.device().enabled);
    assert_eq!(h.f.gpu_environment.authorizations.load(Ordering::SeqCst), 0);
    let result = h.application.keep(h.runtime(), report.id).unwrap();
    assert!(result.targets_applied);
    success(&result.steps);
    assert!(h.f.gpu_environment.io.device().enabled);
    assert_eq!(h.f.device.0.lock().unwrap().gpu_khz, 60_000);
    assert!(
        h.f.display
            .controller
            .read()
            .unwrap()
            .displays
            .iter()
            .all(|d| d.current.frequency == 144)
    );
}
#[test]
fn cancelling_display_trial_never_authorizes_or_enables_gpu() {
    let mut h = Harness::new();
    let display = h.display();
    let pending = h.begin(Some(&display));
    let result = h.application.cancel(h.runtime(), pending.id).unwrap();
    assert!(!result.targets_applied);
    assert!(result.pending.is_none());
    assert!(!h.f.gpu_environment.io.device().enabled);
    assert_eq!(h.f.gpu_environment.authorizations.load(Ordering::SeqCst), 0);
}
#[test]
fn later_gpu_failure_recovers_original_power_cpu_and_kept_display_batch() {
    let mut h = Harness::new();
    let display = h.display();
    h.f.device.0.lock().unwrap().fail_gpu = true;
    let pending = h.begin(Some(&display));
    let report = h.application.keep(h.runtime(), pending.id).unwrap();
    assert!(!report.targets_applied);
    assert!(report.pending.is_none());
    assert!(!h.f.gpu_environment.io.device().enabled);
    assert_eq!(
        i32::from_le_bytes(
            h.f.device.0.lock().unwrap().pages[6][33..37]
                .try_into()
                .unwrap()
        ),
        65
    );
    assert!(
        h.f.display
            .controller
            .read()
            .unwrap()
            .displays
            .iter()
            .all(|d| d.current.frequency == 60)
    );
    assert!(
        report
            .steps
            .iter()
            .any(|s| s.step == "恢复本次独显状态" && s.ok)
    );
}
#[test]
fn startup_uses_same_gpu_pipeline_and_preserves_its_execution_result() {
    let h = Harness::new();
    let mut runtime = h.runtime();
    runtime.origin = crate::profiles::application::Origin::Startup;
    runtime.permission = Some(crate::profiles::application::Permission::new(|| Ok(())));
    let report = h
        .application
        .begin(runtime, &h.file, None, None, Some(&h.review), None)
        .unwrap();
    assert!(report.origin == crate::profiles::application::Origin::Startup);
    assert!(
        report.targets_applied,
        "{:?}",
        report
            .steps
            .iter()
            .map(|step| (&step.step, step.ok, &step.message))
            .collect::<Vec<_>>()
    );
    let saved = h
        .application
        .startup_report(&h.f.display.controller)
        .unwrap()
        .unwrap();
    assert_eq!(saved.id, report.id);
    assert!(saved.targets_applied);
    assert_eq!(h.f.gpu_environment.authorizations.load(Ordering::SeqCst), 1);
}

#[test]
fn cleanup_failure_keeps_confirmed_gpu_and_display_targets() {
    let mut h = Harness::new();
    let display = h.display();
    let pending = h.begin(Some(&display));
    h.f.gpu_environment.io.0.lock().unwrap().fail_finish = true;
    let report = h.application.keep(h.runtime(), pending.id).unwrap();
    assert!(report.targets_applied);
    assert!(report.pending.is_none());
    assert!(
        report
            .steps
            .iter()
            .any(|s| s.step == "独显授权收尾" && !s.ok)
    );
    assert!(h.f.gpu_environment.io.device().enabled);
    assert!(
        h.f.display
            .controller
            .read()
            .unwrap()
            .displays
            .iter()
            .all(|d| d.current.frequency == 144)
    );
}
