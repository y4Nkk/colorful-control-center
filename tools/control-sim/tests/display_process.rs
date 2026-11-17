#[allow(dead_code)]
#[path = "../../../src-tauri/src/controller_lease.rs"]
mod controller_lease;
use std::{
    fs,
    path::Path,
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
        if scenario == "blocked-recovery" {
            fs::write(root.path().join("restore.blocked"), b"blocked").unwrap();
        }
        if scenario == "multi-blocked-recovery" {
            fs::write(root.path().join("restore2.blocked"), b"blocked").unwrap();
        }
        let lease = format!(
            "Local\\ccc-display-test-{}-{}",
            std::process::id(),
            root.path().file_name().unwrap().to_string_lossy()
        );
        let parent = Command::new(env!("CARGO_BIN_EXE_display-fixture"))
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
        let deadline = Instant::now() + Duration::from_secs(9);
        loop {
            if let Ok(text) = fs::read_to_string(self.root.path().join(name)) {
                return text;
            }
            assert!(Instant::now() < deadline, "等待 {name} 超时");
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    fn finished(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(9);
        loop {
            if let Some(status) = self.parent.try_wait().unwrap() {
                assert!(status.success());
                return;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    fn child_exited(&self) {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if controller_lease::ControllerLease::try_acquire(&self.lease)
                .unwrap()
                .is_some()
            {
                return;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_file(self.root.path().join("restore.blocked"));
        let _ = fs::remove_file(self.root.path().join("restore2.blocked"));
        let _ = self.parent.kill();
        let _ = self.parent.wait();
        let deadline = Instant::now() + Duration::from_secs(6);
        while Instant::now() < deadline {
            if controller_lease::ControllerLease::try_acquire(&self.lease)
                .is_ok_and(|lease| lease.is_some())
            {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        eprintln!(
            "隔离显示恢复尚未结束，保留诊断目录：{}",
            self.root.path().display()
        );
        self.root.disable_cleanup(true);
    }
}
fn frequency(root: &Path) -> u64 {
    frequency_file(root, "mode.json")
}
fn frequency_file(root: &Path, file: &str) -> u64 {
    serde_json::from_slice::<serde_json::Value>(&fs::read(root.join(file)).unwrap()).unwrap()
        ["frequency"]
        .as_u64()
        .unwrap()
}

fn geometry(root: &Path) -> (u64, u64, u64) {
    let mode: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("mode.json")).unwrap()).unwrap();
    (
        mode["orientation"].as_u64().unwrap(),
        mode["width"].as_u64().unwrap(),
        mode["height"].as_u64().unwrap(),
    )
}

#[test]
fn rotation_killed_parent_recovers_original_portrait_in_child() {
    let mut f = Fixture::start("rotation-killed-parent");
    f.wait("parent.ready");
    assert_eq!(geometry(f.root.path()), (2, 1920, 1080));
    f.parent.kill().unwrap();
    f.parent.wait().unwrap();
    f.wait("restored");
    f.child_exited();
    assert_eq!(geometry(f.root.path()), (1, 1080, 1920));
}

#[test]
fn rotation_confirmed_keep_preserves_target() {
    let mut f = Fixture::start("rotation-keep");
    assert_eq!(f.wait("parent.result"), "kept");
    f.finished();
    f.child_exited();
    assert_eq!(geometry(f.root.path()), (2, 1920, 1080));
    assert!(!f.root.path().join("restored").exists());
}
#[test]
fn confirmed_keep_never_recovers() {
    let mut f = Fixture::start("keep");
    assert_eq!(f.wait("parent.result"), "kept");
    f.finished();
    f.child_exited();
    assert_eq!(frequency(f.root.path()), 144);
    assert!(!f.root.path().join("restored").exists());
}
#[test]
fn whole_batch_keep_preserves_both_targets() {
    let mut f = Fixture::start("multi-keep");
    assert_eq!(f.wait("parent.result"), "kept");
    f.finished();
    f.child_exited();
    assert_eq!(frequency(f.root.path()), 144);
    assert_eq!(frequency_file(f.root.path(), "mode2.json"), 144);
    assert!(!f.root.path().join("restored").exists());
    assert!(!f.root.path().join("restored2").exists());
}
#[test]
fn parent_killed_recovers_both_displays_in_independent_process() {
    let mut f = Fixture::start("multi-killed-parent");
    f.wait("parent.ready");
    f.parent.kill().unwrap();
    f.parent.wait().unwrap();
    assert_eq!(f.wait("restored"), "60");
    assert_eq!(f.wait("restored2"), "60");
    f.child_exited();
}
#[test]
fn partial_batch_never_writes_unattempted_display_during_recovery() {
    let mut f = Fixture::start("multi-partial");
    assert_eq!(f.wait("parent.result"), "recovered");
    f.finished();
    f.child_exited();
    assert_eq!(frequency(f.root.path()), 60);
    assert!(!f.root.path().join("restore2.attempt").exists());
}
#[test]
fn blocked_second_display_does_not_starve_first_and_retains_lease_for_retry() {
    let mut f = Fixture::start("multi-blocked-recovery");
    f.wait("parent.ready");
    f.parent.kill().unwrap();
    f.parent.wait().unwrap();
    assert_eq!(f.wait("restored"), "60");
    f.wait("restore2.attempt");
    assert!(!f.root.path().join("restored2").exists());
    assert!(
        controller_lease::ControllerLease::try_acquire(&f.lease)
            .unwrap()
            .is_none()
    );
    fs::remove_file(f.root.path().join("restore2.blocked")).unwrap();
    assert_eq!(f.wait("restored2"), "60");
    f.child_exited();
}
#[test]
fn parent_killed_leaves_independent_recovery() {
    let mut f = Fixture::start("killed-parent");
    f.wait("parent.ready");
    f.parent.kill().unwrap();
    f.parent.wait().unwrap();
    assert_eq!(f.wait("restored"), "60");
    f.child_exited();
    assert_eq!(frequency(f.root.path()), 60);
}
#[test]
fn live_parent_with_unconfirmed_trial_times_out() {
    let mut f = Fixture::start("timeout");
    assert_eq!(f.wait("parent.result"), "recovered");
    f.finished();
    f.child_exited();
    assert_eq!(frequency(f.root.path()), 60);
}
#[test]
fn wrong_keep_is_rejected_then_pipe_close_recovers() {
    let mut f = Fixture::start("reject-keep");
    assert_eq!(f.wait("parent.result"), "rejected");
    f.finished();
    assert_eq!(f.wait("restored"), "60");
    f.child_exited();
}
#[test]
fn retrying_recovery_retains_lease_after_parent_death() {
    let mut f = Fixture::start("blocked-recovery");
    f.wait("parent.ready");
    f.parent.kill().unwrap();
    f.parent.wait().unwrap();
    f.wait("restore.attempt");
    assert!(
        controller_lease::ControllerLease::try_acquire(&f.lease)
            .unwrap()
            .is_none()
    );
    fs::remove_file(f.root.path().join("restore.blocked")).unwrap();
    assert_eq!(f.wait("restored"), "60");
    f.child_exited();
}
#[test]
fn ready_timeout_reclaims_unarmed_child_without_target_write() {
    let mut f = Fixture::start("never-ready");
    f.wait("start.rejected");
    f.finished();
    f.child_exited();
    assert_eq!(frequency(f.root.path()), 60);
    assert!(!f.root.path().join("restored").exists());
}
#[test]
fn child_exit_is_detected_and_parent_restores_before_releasing() {
    use windows_sys::Win32::{
        Foundation::CloseHandle,
        System::Threading::{
            OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE,
            QueryFullProcessImageNameW, SYNCHRONIZATION_SYNCHRONIZE, TerminateProcess,
            WaitForSingleObject,
        },
    };
    let mut f = Fixture::start("child-exit");
    f.wait("parent.ready");
    let pid = f.wait("child.pid").parse().unwrap();
    unsafe {
        let handle = OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_TERMINATE | SYNCHRONIZATION_SYNCHRONIZE,
            0,
            pid,
        );
        assert!(!handle.is_null());
        let mut path = [0u16; 32768];
        let mut len = path.len() as u32;
        assert_ne!(
            QueryFullProcessImageNameW(handle, 0, path.as_mut_ptr(), &mut len),
            0
        );
        let actual = std::path::PathBuf::from(String::from_utf16(&path[..len as usize]).unwrap());
        assert_eq!(
            actual.canonicalize().unwrap(),
            Path::new(env!("CARGO_BIN_EXE_display-fixture"))
                .canonicalize()
                .unwrap()
        );
        assert_ne!(TerminateProcess(handle, 1), 0);
        assert_eq!(WaitForSingleObject(handle, 3000), 0);
        CloseHandle(handle);
    }
    fs::write(f.root.path().join("kill.child"), b"terminated").unwrap();
    assert_eq!(f.wait("parent.result"), "fallback");
    f.finished();
    f.child_exited();
    assert_eq!(frequency(f.root.path()), 60);
}
