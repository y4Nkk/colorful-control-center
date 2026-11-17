use super::*;

struct Cleanup<'a>(&'a Hal);
impl Drop for Cleanup<'_> {
    fn drop(&mut self) {
        let _ = fanctl::software::stop(self.0);
        if let Err(error) = self.0.disarm_fan_guard() {
            eprintln!("隔离风扇监护收尾：{error:#}");
        }
    }
}

#[test]
#[ignore = "需先编译control-sim watchdog-fixture并显式设置CCC_TEST_FAN_WATCHDOG"]
fn software_curve_after_gpu_enable_and_later_shutdown_failure_restore_original_curve() {
    let mut f = setup(false);
    let executable = std::path::PathBuf::from(
        std::env::var_os("CCC_TEST_FAN_WATCHDOG").expect("未注入隔离风扇监护"),
    );
    let expected = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../tools/control-sim/target/debug/watchdog-fixture.exe")
        .canonicalize()
        .unwrap();
    assert_eq!(executable.canonicalize().unwrap(), expected);
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().to_owned();
    f.hal.test_fan_guard = Some(Arc::new(move |policy| {
        let mut command = std::process::Command::new(&executable);
        command.arg(&root).arg("disarm");
        crate::watchdog::Watchdog::spawn(command, false, policy)
    }));
    let _cleanup = Cleanup(&f.hal);
    let mut settings = wanted(&f, true);
    let original = curves();
    settings.fan_software_curves = Some(original.clone());
    success(&apply(&f, &settings));
    assert!(f.gpu_environment.io.device().enabled);
    assert_eq!(
        fanctl::software::read(&f.hal).unwrap(),
        Some(original.clone())
    );
    f.display.enable();
    let display = f.display.controller.capture().unwrap().unwrap();
    let io = f.display.io.clone();
    f.gpu_environment.io.0.lock().unwrap().node_hook = Some(Arc::new(move |enabled| {
        if !enabled {
            io.0.lock().unwrap().displays[0].current.frequency = 144;
        }
    }));
    settings = wanted(&f, false);
    settings.display = Some(display);
    let steps = apply(&f, &settings);
    assert!(steps.iter().any(|s| !s.ok && s.step == "显示刷新率复核"));
    assert!(f.gpu_environment.io.device().enabled);
    assert_eq!(fanctl::software::read(&f.hal).unwrap(), Some(original));
    fanctl::software::stop(&f.hal).unwrap();
    f.hal.disarm_fan_guard().unwrap();
}
