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
    directory: Option<tempfile::TempDir>,
}
impl Fixture {
    fn start(scenario: &str) -> Self {
        let directory = tempfile::tempdir().unwrap();
        let parent = Command::new(env!("CARGO_BIN_EXE_lan-authorization-fixture"))
            .arg("parent")
            .arg(directory.path())
            .arg(scenario)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        Self {
            parent,
            directory: Some(directory),
        }
    }
    fn root(&self) -> &Path {
        self.directory.as_ref().unwrap().path()
    }
    fn wait(&self, name: &str) -> String {
        let deadline = Instant::now() + Duration::from_secs(9);
        loop {
            if let Ok(text) = fs::read_to_string(self.root().join(name)) {
                return text;
            }
            assert!(
                Instant::now() < deadline,
                "等待{name}，日志：{}",
                self.root().display()
            );
            std::thread::sleep(Duration::from_millis(15));
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
            std::thread::sleep(Duration::from_millis(15));
        }
    }
    fn child_exited(&self) {
        self.wait("child.exit");
        let pid = self.wait("child.pid").parse().unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while process::alive(pid) {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(15));
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.parent.kill();
        let _ = self.parent.wait();
        if std::thread::panicking() {
            let path = self.directory.take().unwrap().keep();
            eprintln!("保留LAN隔离进程诊断：{}", path.display());
            return;
        }
        if self.root().join("child.pid").exists() {
            self.child_exited();
        }
    }
}
#[test]
fn lan_process_ready_waits_for_explicit_commit_before_execution() {
    let mut f = Fixture::start("commit");
    f.wait("parent.ready");
    assert!(!f.root().join("executed").exists());
    fs::write(f.root().join("continue"), b"commit").unwrap();
    f.finished();
    f.child_exited();
    assert_eq!(f.wait("executed"), "only-after-commit");
    assert!(!f.root().join("cancelled").exists());
}
#[test]
fn lan_process_cancel_reaps_child_without_execution() {
    let mut f = Fixture::start("cancel");
    f.wait("parent.ready");
    fs::write(f.root().join("continue"), b"cancel").unwrap();
    f.finished();
    f.child_exited();
    assert!(f.wait("cancelled").contains("取消"));
    assert!(!f.root().join("executed").exists());
}
#[test]
fn lan_process_parent_kill_causes_child_to_exit_without_execution() {
    let mut f = Fixture::start("parent-death");
    f.wait("parent.ready");
    f.parent.kill().unwrap();
    f.parent.wait().unwrap();
    f.child_exited();
    assert!(f.wait("cancelled").contains("主进程已退出"));
    assert!(!f.root().join("executed").exists());
}
#[test]
fn lan_process_early_exit_does_not_publish_ready_or_execute() {
    let mut f = Fixture::start("exit-before-ready");
    f.finished();
    assert!(f.wait("refused").contains("未就绪"));
    assert!(!f.root().join("executed").exists());
    assert!(!f.root().join("parent.ready").exists());
}

#[test]
fn lan_process_same_authorized_child_handles_apply_rollback_and_finish() {
    let mut f = Fixture::start("rollback");
    f.wait("parent.ready");
    let pid = f.wait("child.pid");
    assert!(!f.root().join("task.prepared").exists());
    fs::write(f.root().join("continue"), b"rollback").unwrap();
    f.finished();
    f.child_exited();
    assert_eq!(f.wait("child.pid"), pid);
    assert_eq!(f.wait("value"), "0");
    assert_eq!(f.wait("restored"), "same-authorized-process");
}
#[test]
fn lan_process_parent_kill_after_apply_restores_the_attempted_targets() {
    let mut f = Fixture::start("parent-death-after-execute");
    f.wait("parent.applied");
    assert_eq!(f.wait("value"), "1");
    f.parent.kill().unwrap();
    f.parent.wait().unwrap();
    f.child_exited();
    assert_eq!(f.wait("value"), "0");
    assert_eq!(f.wait("restored"), "same-authorized-process");
}
