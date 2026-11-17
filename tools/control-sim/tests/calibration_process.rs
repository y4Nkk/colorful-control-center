#[allow(dead_code)]
#[path = "../../../src-tauri/src/controller_lease.rs"]
mod controller_lease;
use controller_lease::ControllerLease;
use serde_json::Value;
use std::{
    fs,
    path::Path,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
use tempfile::TempDir;

struct Fixture {
    root: Option<TempDir>,
    parent: Child,
    lease: String,
}
impl Fixture {
    fn start(scenario: &str) -> Self {
        let root = tempfile::Builder::new()
            .prefix("ccc-calibration-")
            .tempdir()
            .unwrap();
        let lease = format!(
            "Local\\ccc-calibration-test-{}-{}",
            std::process::id(),
            root.path().file_name().unwrap().to_string_lossy()
        );
        fs::write(
            root.path().join("fixture.marker"),
            b"ccc-calibration-fixture",
        )
        .unwrap();
        fs::write(root.path().join("lease.name"), &lease).unwrap();
        if scenario == "blocked-recovery" {
            fs::write(root.path().join("restore.blocked"), b"blocked").unwrap();
        }
        if scenario == "import-failure" {
            fs::write(root.path().join("import.partial"), b"partial").unwrap();
        }
        let mut command = Command::new(env!("CARGO_BIN_EXE_calibration-fixture"));
        command
            .arg("parent")
            .arg(root.path())
            .arg(&lease)
            .arg(scenario)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        Self {
            parent: command.spawn().unwrap(),
            root: Some(root),
            lease,
        }
    }
    fn root(&self) -> &Path {
        self.root.as_ref().unwrap().path()
    }
    fn wait(&self, name: &str) {
        let deadline = Instant::now() + Duration::from_secs(12);
        while Instant::now() < deadline {
            if self.root().join(name).exists() {
                return;
            }
            std::thread::sleep(Duration::from_millis(30));
        }
        panic!("未出现 {name}，隔离证据：{}", self.root().display());
    }
    fn parent_exit(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(12);
        while Instant::now() < deadline {
            if let Some(status) = self.parent.try_wait().unwrap() {
                assert!(status.success(), "{status}");
                return;
            }
            std::thread::sleep(Duration::from_millis(30));
        }
        panic!("隔离父进程未结束：{}", self.root().display());
    }
    fn json(&self, name: &str) -> Value {
        serde_json::from_slice(&fs::read(self.root().join(name)).unwrap()).unwrap()
    }
    fn released(&self) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if ControllerLease::try_acquire(&self.lease).unwrap().is_some() {
                return;
            }
            std::thread::sleep(Duration::from_millis(30));
        }
        panic!("隔离控制权未释放：{}", self.root().display());
    }
    fn restored(&self, outcome: &str) {
        let record = self.json("battery-calibration.json");
        assert_eq!(record["pending"], false);
        assert_eq!(record["outcome"], outcome);
        assert_eq!(
            self.json("hardware.json"),
            serde_json::json!({"ac":true,"refresh":false,"flexi":true})
        );
        assert_eq!(
            self.json("charge.json"),
            serde_json::json!({"enabled":true,"start":40,"stop":80})
        );
        assert_eq!(self.json("awake.json"), false);
        let power = self.json("power.json");
        assert_eq!(power["active"], "381b4222-f694-41f0-9685-ff5bb260df2e");
        assert_eq!(power["plans"].as_object().unwrap().len(), 1);
        assert!(!self.root().join("copy.pow").exists());
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_file(self.root().join("restore.blocked"));
        if self.parent.try_wait().ok().flatten().is_none() {
            let _ = self.parent.kill();
        }
        let _ = self.parent.wait();
        let deadline = Instant::now() + Duration::from_secs(6);
        let mut released = false;
        while Instant::now() < deadline {
            if ControllerLease::try_acquire(&self.lease)
                .ok()
                .flatten()
                .is_some()
            {
                released = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        if !released || std::thread::panicking() {
            if let Some(root) = self.root.take() {
                eprintln!("保留隔离证据：{}", root.keep().display());
            }
        }
    }
}
#[test]
fn parent_strong_kill_restores_charge_power_controls_and_releases_lease() {
    let mut fixture = Fixture::start("parent-kill");
    fixture.wait("worker.active");
    assert!(
        ControllerLease::try_acquire(&fixture.lease)
            .unwrap()
            .is_none()
    );
    fixture.parent.kill().unwrap();
    fixture.parent.wait().unwrap();
    fixture.wait("child.exit");
    fixture.restored("failed");
    fixture.released();
}
#[test]
fn restoration_retry_keeps_control_lease_after_parent_is_gone() {
    let mut fixture = Fixture::start("blocked-recovery");
    fixture.wait("worker.active");
    fixture.parent.kill().unwrap();
    fixture.parent.wait().unwrap();
    fixture.wait("restore.attempt");
    assert!(
        ControllerLease::try_acquire(&fixture.lease)
            .unwrap()
            .is_none()
    );
    assert_eq!(fixture.json("battery-calibration.json")["pending"], true);
    fs::remove_file(fixture.root().join("restore.blocked")).unwrap();
    fixture.wait("child.exit");
    fixture.restored("failed");
    fixture.released();
}
#[test]
fn child_early_exit_uses_parent_recovery_transaction() {
    let mut fixture = Fixture::start("child-exit");
    fixture.wait("parent.exit");
    fixture.parent_exit();
    fixture.restored("failed");
    fixture.released();
}
#[test]
fn stopped_heartbeat_on_live_parent_pipe_triggers_independent_recovery() {
    let mut fixture = Fixture::start("heartbeat-loss");
    fixture.wait("child.exit");
    fixture.parent_exit();
    fixture.restored("failed");
    fixture.released();
}
#[test]
fn explicit_stop_and_all_five_stages_finish_with_original_plan() {
    for (scenario, outcome) in [("stop", "stopped"), ("complete", "completed")] {
        let mut fixture = Fixture::start(scenario);
        fixture.wait("child.exit");
        fixture.parent_exit();
        fixture.restored(outcome);
        fixture.released();
    }
}
#[test]
fn changed_plan_and_partial_import_are_compensated_in_real_processes() {
    for scenario in ["plan-change", "import-failure"] {
        let mut fixture = Fixture::start(scenario);
        fixture.wait("child.exit");
        fixture.parent_exit();
        fixture.restored("failed");
        fixture.released();
    }
}
#[test]
fn ready_timeout_never_writes_controls_or_imports_a_plan() {
    let mut fixture = Fixture::start("never-ready");
    fixture.wait("start.rejected");
    fixture.parent_exit();
    assert!(!fixture.root().join("control.log").exists());
    assert!(!fixture.root().join("copy.pow").exists());
    assert_eq!(fixture.json("battery-calibration.json")["armed"], false);
    assert_eq!(fixture.json("battery-calibration.json")["pending"], false);
    assert_eq!(
        fixture.json("power.json")["plans"]
            .as_object()
            .unwrap()
            .len(),
        1
    );
    fixture.released();
}
