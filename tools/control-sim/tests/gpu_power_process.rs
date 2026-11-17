#[allow(dead_code)]
#[path = "../../../src-tauri/src/controller_lease.rs"]
mod controller_lease;
use std::{
    fs,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
use tempfile::TempDir;
struct Fixture {
    root: TempDir,
    parent: Child,
    lease: String,
}
impl Fixture {
    fn start(scenario: &str) -> Self {
        let root = tempfile::tempdir().unwrap();
        let lease = format!(
            "Local\\ccc-gpu-power-{}-{}",
            std::process::id(),
            root.path().file_name().unwrap().to_string_lossy()
        );
        let parent = Command::new(env!("CARGO_BIN_EXE_gpu-power-fixture"))
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
            root,
            parent,
            lease,
        }
    }
    fn wait(&self, name: &str) -> String {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Ok(text) = fs::read_to_string(self.root.path().join(name)) {
                return text;
            }
            assert!(Instant::now() < deadline, "等待{name}超时");
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    fn json(&self, name: &str) -> serde_json::Value {
        serde_json::from_str(&self.wait(name)).unwrap()
    }
    fn finish(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(status) = self.parent.try_wait().unwrap() {
                assert!(status.success());
                return;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    fn released(&self) {
        let deadline = Instant::now() + Duration::from_secs(6);
        while controller_lease::ControllerLease::try_acquire(&self.lease)
            .unwrap()
            .is_none()
        {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::write(self.root.path().join("continue"), b"continue");
        let _ = fs::write(self.root.path().join("retry"), b"retry");
        let _ = self.parent.kill();
        let _ = self.parent.wait();
        let deadline = Instant::now() + Duration::from_secs(6);
        while Instant::now() < deadline {
            if controller_lease::ControllerLease::try_acquire(&self.lease)
                .is_ok_and(|lease| lease.is_some())
            {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        self.root.disable_cleanup(true);
        eprintln!(
            "GPU隔离工作进程尚未释放，保留诊断：{}",
            self.root.path().display()
        );
    }
}
#[test]
fn actual_child_commits_original_order_and_full_target() {
    let mut f = Fixture::start("success");
    assert_eq!(f.json("parent.result")["code"], 0);
    f.finish();
    f.released();
    let state = f.json("state.json");
    assert_eq!(state["device"]["enabled"], false);
    assert_eq!(state["device"]["automaticPowerOff"], true);
    assert_eq!(state["firmware"], false);
    assert_eq!(
        f.json("events.json"),
        serde_json::json!([
            "node:false",
            "node:false",
            "node:false",
            "firmware:false",
            "automatic:true"
        ])
    );
}
#[test]
fn parent_killed_mid_operation_keeps_lease_until_worker_finishes() {
    let mut f = Fixture::start("hold");
    f.wait("parent.ready");
    f.wait("worker.paused");
    f.parent.kill().unwrap();
    f.parent.wait().unwrap();
    assert!(
        controller_lease::ControllerLease::try_acquire(&f.lease)
            .unwrap()
            .is_none()
    );
    fs::write(f.root.path().join("continue"), b"continue").unwrap();
    assert_eq!(f.json("child.result")["code"], 0);
    f.released();
    let state = f.json("state.json");
    assert_eq!(state["device"]["automaticPowerOff"], true);
    assert_eq!(state["firmware"], false);
}
#[test]
fn worker_failure_compensates_before_exiting_with_stage_code() {
    let mut f = Fixture::start("compensate");
    let code = f.json("parent.result")["code"].as_u64().unwrap();
    assert_eq!((code >> 16) & 127, 9);
    assert_eq!(code & 0x00800000, 0);
    f.finish();
    f.released();
    let state = f.json("state.json");
    assert_eq!(state["device"]["enabled"], true);
    assert_eq!(state["device"]["automaticPowerOff"], false);
    assert_eq!(state["firmware"], true);
}
#[test]
fn failed_compensation_is_reported_and_explicit_retry_restores_original() {
    let mut f = Fixture::start("retry");
    let code = f.json("first.result")["code"].as_u64().unwrap();
    assert_ne!(code & 0x00800000, 0);
    assert_eq!((code >> 16) & 127, 10);
    fs::write(f.root.path().join("retry"), b"confirmed retry").unwrap();
    assert_eq!(f.json("parent.result")["code"], 0);
    f.finish();
    f.released();
    let state = f.json("state.json");
    assert_eq!(state["device"]["enabled"], true);
    assert_eq!(state["device"]["automaticPowerOff"], false);
    assert_eq!(state["firmware"], true);
}
