use super::*;
use crate::profiles::{
    ProfileFile,
    application::{Controller, Runtime},
};
use std::path::Path;

fn runtime<'a>(f: &'a Fixture, dir: &'a Path, startup: bool) -> Runtime<'a> {
    Runtime {
        origin: if startup {
            crate::profiles::application::Origin::Startup
        } else {
            crate::profiles::application::Origin::Manual
        },
        permission: startup.then(|| crate::profiles::application::Permission::new(|| Ok(()))),
        hal: &f.hal,
        gpu: &f.gpu,
        devices: f.devices(),
        dir,
    }
}
fn file(f: &Fixture, dir: &Path) -> ProfileFile {
    let file = ProfileFile {
        version: 1,
        name: "多区组合".into(),
        saved_at_ms: 1,
        settings: ProfileSettings {
            multi_zone_lighting: Some(fixture::target(f)),
            ..Default::default()
        },
    };
    save(&file, dir);
    file
}
fn save(file: &ProfileFile, dir: &Path) {
    std::fs::write(dir.join("多区组合.json"), serde_json::to_vec(file).unwrap()).unwrap();
}

#[test]
fn manual_and_startup_apply_the_same_multizone_target() {
    for startup in [false, true] {
        let mut f = Fixture::new();
        let io = fixture::enable(&mut f);
        let dir = tempfile::tempdir().unwrap();
        let file = file(&f, dir.path());
        let application = Controller::default();
        let report = application
            .begin(
                runtime(&f, dir.path(), startup),
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
        assert!(report.steps.iter().all(|step| step.ok));
        assert_eq!(io.traffic.lock().unwrap().packets.len(), 1);
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
}
#[test]
fn changed_preview_file_is_rejected_before_any_multizone_output() {
    let mut f = Fixture::new();
    let io = fixture::enable(&mut f);
    let dir = tempfile::tempdir().unwrap();
    let file = file(&f, dir.path());
    let mut changed = file.clone();
    changed
        .settings
        .multi_zone_lighting
        .as_mut()
        .unwrap()
        .values
        .mode = 7;
    save(&changed, dir.path());
    let report = Controller::default()
        .begin(
            runtime(&f, dir.path(), false),
            &file,
            None,
            None,
            None,
            None,
        )
        .unwrap();
    assert!(!report.targets_applied);
    assert!(io.traffic.lock().unwrap().packets.is_empty());
}
#[test]
fn display_confirmation_and_capability_recheck_precede_multizone_output() {
    for action in 0..3 {
        let mut f = Fixture::new();
        let io = fixture::enable(&mut f);
        f.display.enable();
        let dir = tempfile::tempdir().unwrap();
        let mut file = file(&f, dir.path());
        let mut target = f.display.controller.capture().unwrap().unwrap();
        for mode in &mut target.displays {
            mode.frequency = 144;
        }
        let display = f.display.controller.preview_profile(&target, None).unwrap();
        file.settings.display = Some(target);
        save(&file, dir.path());
        let application = Controller::default();
        let report = application
            .begin(
                runtime(&f, dir.path(), false),
                &file,
                None,
                Some(&display),
                None,
                None,
            )
            .unwrap();
        assert!(report.pending.is_some());
        assert!(io.traffic.lock().unwrap().packets.is_empty());
        if action == 2 {
            f.device.0.lock().unwrap().bios[23] = 0;
        }
        let report = if action == 0 {
            application
                .cancel(runtime(&f, dir.path(), false), report.id)
                .unwrap()
        } else {
            application
                .keep(runtime(&f, dir.path(), false), report.id)
                .unwrap()
        };
        assert_eq!(report.targets_applied, action == 1);
        assert_eq!(
            io.traffic.lock().unwrap().packets.len(),
            usize::from(action == 1)
        );
    }
}
