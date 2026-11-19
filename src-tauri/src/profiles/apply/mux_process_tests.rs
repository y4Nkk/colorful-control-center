use super::*;

struct Cleanup<'a>(&'a Hal);
impl Drop for Cleanup<'_> {
    fn drop(&mut self) {
        let _ = fanctl::software::stop(self.0);
        if let Err(error) = self.0.disarm_fan_guard() {
            eprintln!("隔离MUX风扇监护收尾：{error:#}");
        }
    }
}
fn guard(f: &mut Fixture) -> tempfile::TempDir {
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
    f.hal.test_fan_guard = Some(Arc::new(move || {
        let mut command = std::process::Command::new(&executable);
        command.arg(&root).arg("disarm");
        crate::watchdog::Watchdog::spawn(command)
    }));
    directory
}
fn curves() -> ccc_core::software_fan::Curves {
    ["cpu", "gpu1"]
        .into_iter()
        .map(|id| {
            (
                id.into(),
                ccc_core::fan::FanCurve {
                    points: vec![
                        ccc_core::fan::CurvePoint {
                            temp_c: 40.0,
                            duty_pct: 30.0,
                        },
                        ccc_core::fan::CurvePoint {
                            temp_c: 95.0,
                            duty_pct: 100.0,
                        },
                    ],
                    hysteresis_c: 3.0,
                    ramp_pct_per_s: 20.0,
                },
            )
        })
        .collect()
}
fn assert_success(steps: &[ApplyStep]) {
    assert!(
        steps.iter().all(|step| step.ok),
        "{}",
        steps
            .iter()
            .filter(|step| !step.ok)
            .map(|step| format!("{}：{}", step.step, step.message))
            .collect::<Vec<_>>()
            .join("；")
    );
}

#[test]
#[ignore = "需显式注入control-sim watchdog-fixture，禁止生产监护回退"]
fn mux_success_partial_failure_and_late_failure_preserve_software_curve() {
    let mut f = setup();
    f.enable_gpu_power();
    let _directory = guard(&mut f);
    let _cleanup = Cleanup(&f.hal);
    let original = curves();
    let mut settings = target(&f, 2);
    settings.fan_software_curves = Some(original.clone());
    assert_success(&apply_settings(&f.hal, &f.gpu, f.devices(), &settings).unwrap());
    assert_eq!(
        fanctl::software::read(&f.hal).unwrap(),
        Some(original.clone())
    );
    assert_eq!(f.graphics_mode.io.0.lock().unwrap().sent, [2]);

    settings = target(&f, 4);
    f.graphics_mode.io.0.lock().unwrap().fail_send = vec![2];
    let steps = apply_settings(&f.hal, &f.gpu, f.devices(), &settings).unwrap();
    assert!(
        steps
            .iter()
            .any(|step| !step.ok && step.step.contains("显卡模式"))
    );
    assert_eq!(f.graphics_mode.io.0.lock().unwrap().sent, [2, 4, 2]);
    assert_eq!(
        fanctl::software::read(&f.hal).unwrap(),
        Some(original.clone())
    );

    f.display.enable();
    settings.display = f.display.controller.capture().unwrap();
    let display = f.display.io.clone();
    f.graphics_mode.io.0.lock().unwrap().hook = Some(Arc::new(move |mode| {
        if mode == 4 {
            display.0.lock().unwrap().displays[0].current.frequency = 144;
        }
    }));
    let steps = apply_settings(&f.hal, &f.gpu, f.devices(), &settings).unwrap();
    assert!(
        steps
            .iter()
            .any(|step| !step.ok && step.step == "显示刷新率复核")
    );
    assert_eq!(f.graphics_mode.io.0.lock().unwrap().sent, [2, 4, 2, 4, 2]);
    assert_eq!(fanctl::software::read(&f.hal).unwrap(), Some(original));
    assert_eq!(f.graphics_mode.io.0.lock().unwrap().reboots, 0);
}
