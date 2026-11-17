#[path = "../../../src-tauri/src/controller_lease.rs"]
#[allow(dead_code)]
mod controller_lease;
#[path = "support/process.rs"]
mod process;
use std::{
    fs,
    path::Path,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

struct Fixture {
    parent: Child,
    root: tempfile::TempDir,
    lease: String,
}
impl Fixture {
    fn start(scenario: &str) -> Self {
        let root = tempfile::tempdir().unwrap();
        let lease = format!(
            "Local\\ccc-gpu-session-{}-{}",
            std::process::id(),
            root.path().file_name().unwrap().to_string_lossy()
        );
        let parent = Command::new(env!("CARGO_BIN_EXE_gpu-session-fixture"))
            .arg("parent")
            .arg(root.path())
            .arg(&lease)
            .arg(scenario)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        Self {
            parent,
            root,
            lease,
        }
    }
    fn wait(&self, name: &str) -> String {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Ok(value) = fs::read_to_string(self.root.path().join(name)) {
                return value;
            }
            assert!(
                Instant::now() < deadline,
                "等待{name}超时：{}",
                self.root.path().display()
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    fn state(&self) -> serde_json::Value {
        serde_json::from_str(&self.wait("state.json")).unwrap()
    }
    fn proceed(&self) {
        fs::write(self.root.path().join("continue"), b"continue").unwrap();
    }
    fn reap(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while self.parent.try_wait().unwrap().is_none() {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(10));
        }
        self.wait("child.exit");
        let child = self.wait("child.pid").parse().unwrap();
        while process::alive(child) {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(
            controller_lease::ControllerLease::try_acquire(&self.lease)
                .unwrap()
                .is_some()
        );
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.proceed();
        let _ = self.parent.kill();
        let _ = self.parent.wait();
        if std::thread::panicking() {
            return;
        }
        self.reap();
        let path = std::path::PathBuf::from(self.wait("session.path"));
        if let Ok(target) = path.canonicalize() {
            let temporary = std::env::temp_dir().canonicalize().unwrap();
            assert_eq!(target.parent(), Some(temporary.as_path()));
            assert!(
                target
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .starts_with(".gpu-session-")
            );
            fs::remove_dir_all(target).unwrap();
        }
    }
}
#[test]
fn ready_never_prepares_devices_before_explicit_permission() {
    let mut f = Fixture::start("commit");
    f.wait("parent.ready");
    assert!(!f.root.path().join("task.prepared").exists());
    assert_eq!(f.state()["events"], serde_json::json!([]));
    f.proceed();
    f.wait("parent.exit");
    f.reap();
    assert_eq!(f.state()["device"]["enabled"], false);
    assert_eq!(f.state()["events"].as_array().unwrap().len(), 5);
}
#[test]
fn later_failure_restores_in_same_authorized_child() {
    let mut f = Fixture::start("rollback");
    f.wait("parent.ready");
    f.proceed();
    f.wait("parent.exit");
    f.reap();
    assert_eq!(f.state()["device"]["enabled"], true);
    assert_eq!(f.state()["device"]["automaticPowerOff"], false);
    assert_eq!(f.state()["firmware"], true);
    assert_eq!(f.state()["events"].as_array().unwrap().len(), 8);
}
#[test]
fn cancellation_and_parent_death_before_permission_have_zero_outputs() {
    for scenario in ["cancel", "death-ready"] {
        let mut f = Fixture::start(scenario);
        f.wait("parent.ready");
        if scenario == "cancel" {
            f.proceed();
            f.wait("parent.exit");
        } else {
            f.parent.kill().unwrap();
            f.parent.wait().unwrap();
        }
        f.reap();
        assert!(!f.root.path().join("task.prepared").exists());
        assert_eq!(f.state()["events"], serde_json::json!([]));
    }
}
#[test]
fn parent_death_after_execution_recovers_and_releases_control_lease() {
    let mut f = Fixture::start("death-applied");
    f.wait("parent.ready");
    assert_eq!(f.state()["device"]["enabled"], false);
    f.parent.kill().unwrap();
    f.parent.wait().unwrap();
    f.reap();
    assert_eq!(f.state()["device"]["enabled"], true);
    assert_eq!(f.state()["firmware"], true);
}
#[test]
fn parent_death_mid_write_keeps_lease_until_transaction_and_recovery_end() {
    let mut f = Fixture::start("death-during-execute");
    f.wait("worker.paused");
    f.parent.kill().unwrap();
    f.parent.wait().unwrap();
    assert!(
        controller_lease::ControllerLease::try_acquire(&f.lease)
            .unwrap()
            .is_none()
    );
    f.proceed();
    f.reap();
    assert_eq!(f.state()["device"]["enabled"], true);
    assert_eq!(f.state()["device"]["automaticPowerOff"], false);
    assert_eq!(f.state()["firmware"], true);
}
#[test]
fn mutated_request_after_ready_is_refused_without_device_preparation() {
    let mut f = Fixture::start("changed-request");
    f.wait("parent.ready");
    let session = f.wait("session.path");
    fs::write(Path::new(&session).join("request.json"), b"{}").unwrap();
    f.proceed();
    assert!(f.wait("parent.error").contains("请求复核失败"));
    f.wait("parent.exit");
    f.reap();
    assert!(!f.root.path().join("task.prepared").exists());
    assert_eq!(f.state()["events"], serde_json::json!([]));
}

#[test]
fn parent_death_between_sequence_stages_recovers_every_stage_in_one_child() {
    let mut f = Fixture::start("sequence-death-between");
    f.wait("parent.ready");
    assert_eq!(f.state()["device"]["automaticPowerOff"], true);
    f.parent.kill().unwrap();
    f.parent.wait().unwrap();
    f.reap();
    assert_eq!(f.state()["device"]["enabled"], true);
    assert_eq!(f.state()["device"]["automaticPowerOff"], false);
    assert_eq!(f.state()["firmware"], true);
}

#[test]
fn death_during_third_sequence_stage_holds_lease_until_reverse_recovery_completes() {
    let mut f = Fixture::start("sequence-death-third");
    f.wait("worker.paused");
    f.parent.kill().unwrap();
    f.parent.wait().unwrap();
    assert!(
        controller_lease::ControllerLease::try_acquire(&f.lease)
            .unwrap()
            .is_none()
    );
    f.proceed();
    f.reap();
    assert_eq!(f.state()["device"]["enabled"], true);
    assert_eq!(f.state()["device"]["automaticPowerOff"], false);
    assert_eq!(f.state()["firmware"], true);
}

#[test]
fn prematurely_finishing_a_sequence_restores_every_completed_stage() {
    let mut f = Fixture::start("sequence-partial-finish");
    assert!(f.wait("parent.error").contains("不能提交部分目标"));
    f.wait("parent.exit");
    f.reap();
    assert_eq!(f.state()["device"]["enabled"], true);
    assert_eq!(f.state()["device"]["automaticPowerOff"], false);
    assert_eq!(f.state()["firmware"], true);
}
