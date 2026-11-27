use super::*;
use crate::profiles::{
    ProfileFile,
    application::{Application, Controller, Runtime},
};

struct Harness {
    f: Fixture,
    dir: tempfile::TempDir,
    file: ProfileFile,
    application: Controller,
    review: graphics_mode::profile::Review,
}
impl Harness {
    fn new() -> Self {
        let f = setup();
        let settings = target(&f, 2);
        let review = review(&f, &settings);
        let h = Self {
            f,
            dir: tempfile::tempdir().unwrap(),
            application: Controller::default(),
            file: ProfileFile {
                version: 1,
                name: "显卡选择组合".into(),
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
            self.dir.path().join("显卡选择组合.json"),
            serde_json::to_vec(&self.file).unwrap(),
        )
        .unwrap();
    }
    fn runtime(&self, startup: bool) -> Runtime<'_> {
        Runtime {
            origin: if startup {
                crate::profiles::application::Origin::Startup
            } else {
                crate::profiles::application::Origin::Manual
            },
            permission: startup.then(|| crate::profiles::application::Permission::new(|| Ok(()))),
            hal: &self.f.hal,
            gpu: &self.f.gpu,
            devices: self.f.devices(),
            dir: self.dir.path(),
        }
    }
    fn begin(
        &self,
        startup: bool,
        display: Option<&crate::display::profile::Review>,
        mux: Option<&graphics_mode::profile::Review>,
    ) -> Application {
        self.application
            .begin(self.runtime(startup), &self.file, None, display, None, mux)
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
fn manual_application_requires_mux_review_before_any_write() {
    let h = Harness::new();
    let result = h.begin(false, None, None);
    assert!(!result.targets_applied);
    assert!(
        result
            .steps
            .iter()
            .any(|s| !s.ok && s.message.contains("显卡模式预览"))
    );
    assert!(h.f.device.0.lock().unwrap().events.is_empty());
    assert!(h.f.graphics_mode.io.0.lock().unwrap().sent.is_empty());
}

#[test]
fn startup_applies_selection_without_requesting_reboot() {
    let h = Harness::new();
    let result = h.begin(true, None, Some(&h.review));
    assert!(
        result.origin == crate::profiles::application::Origin::Startup && result.targets_applied
    );
    assert!(result.steps.iter().all(|s| s.ok));
    assert_eq!(selection(&h.f).last_request.unwrap().target, 2);
    assert_eq!(h.f.graphics_mode.io.0.lock().unwrap().reboots, 0);
    assert!(
        h.application
            .startup_report(&h.f.display.controller)
            .unwrap()
            .unwrap()
            .targets_applied
    );
}

#[test]
fn stale_boot_and_foreign_choice_invalidate_manual_confirmation() {
    for change in 0..2 {
        let h = Harness::new();
        if change == 0 {
            h.f.graphics_mode.io.0.lock().unwrap().identity.boot = "2".repeat(32);
        } else {
            request(&h.f, 4);
        }
        let before = h.f.graphics_mode.io.0.lock().unwrap().sent.clone();
        let result = h.begin(false, None, Some(&h.review));
        assert!(!result.targets_applied);
        assert!(h.f.device.0.lock().unwrap().events.is_empty());
        assert_eq!(h.f.graphics_mode.io.0.lock().unwrap().sent, before);
    }
}

#[test]
fn display_is_kept_before_mux_and_cancellation_sends_no_choice() {
    for keep in [false, true] {
        let mut h = Harness::new();
        let display = h.display();
        let report = h.begin(false, Some(&display), Some(&h.review));
        assert!(report.pending.is_some());
        assert!(h.f.graphics_mode.io.0.lock().unwrap().sent.is_empty());
        let report = if keep {
            h.application
                .keep(&h.f.hal, &h.f.gpu, h.f.devices(), report.id)
                .unwrap()
        } else {
            h.application
                .cancel(&h.f.hal, &h.f.display.controller, report.id)
                .unwrap()
        };
        assert_eq!(report.targets_applied, keep);
        assert_eq!(
            h.f.graphics_mode.io.0.lock().unwrap().sent.len(),
            usize::from(keep)
        );
        assert_eq!(h.f.graphics_mode.io.0.lock().unwrap().reboots, 0);
    }
}

#[test]
fn source_change_during_display_trial_restores_displays_and_prevents_mux() {
    let mut h = Harness::new();
    let display = h.display();
    let report = h.begin(false, Some(&display), Some(&h.review));
    h.f.graphics_mode.io.0.lock().unwrap().identity.boot = "2".repeat(32);
    let report = h
        .application
        .keep(&h.f.hal, &h.f.gpu, h.f.devices(), report.id)
        .unwrap();
    assert!(!report.targets_applied && report.pending.is_none());
    assert!(h.f.graphics_mode.io.0.lock().unwrap().sent.is_empty());
    assert!(
        h.f.display
            .io
            .0
            .lock()
            .unwrap()
            .displays
            .iter()
            .all(|d| d.current.frequency == 60)
    );
}

#[test]
fn previewed_file_change_prevents_all_targets() {
    let h = Harness::new();
    let mut file = h.file.clone();
    file.settings.graphics_mode.as_mut().unwrap().mode = 4;
    std::fs::write(
        h.dir.path().join("显卡选择组合.json"),
        serde_json::to_vec(&file).unwrap(),
    )
    .unwrap();
    let report = h.begin(false, None, Some(&h.review));
    assert!(!report.targets_applied);
    assert!(h.f.device.0.lock().unwrap().events.is_empty());
    assert!(h.f.graphics_mode.io.0.lock().unwrap().sent.is_empty());
}
