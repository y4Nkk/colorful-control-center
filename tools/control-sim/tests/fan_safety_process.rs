//! 真实子进程强杀与重新读取，仅访问专用临时恢复记录。
use std::{
    fs,
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

struct Fixture {
    dir: Option<tempfile::TempDir>,
    child: Option<Child>,
}
impl Fixture {
    fn new() -> Self {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/fan-safety-tests");
        fs::create_dir_all(&root).unwrap();
        Self {
            dir: Some(
                tempfile::Builder::new()
                    .prefix("journal-")
                    .tempdir_in(root)
                    .unwrap(),
            ),
            child: None,
        }
    }
    fn path(&self) -> PathBuf {
        self.dir.as_ref().unwrap().path().to_owned()
    }
    fn command(&self, action: &str) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_fan-safety-fixture"));
        command
            .arg(self.path())
            .arg(action)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command
    }
    fn succeeds(&self, action: &str) -> bool {
        let output = self.command(action).output().unwrap();
        fs::write(self.path().join(format!("{action}.stderr")), &output.stderr).unwrap();
        output.status.success()
    }
    fn kill_running_writer(&mut self) {
        self.child = Some(self.command("armed").spawn().unwrap());
        let deadline = Instant::now() + Duration::from_secs(8);
        while !self.path().join("ready").exists() {
            assert!(
                Instant::now() < deadline,
                "运行标记超时：{}",
                self.path().display()
            );
            assert!(self.child.as_mut().unwrap().try_wait().unwrap().is_none());
            std::thread::sleep(Duration::from_millis(10));
        }
        let mut child = self.child.take().unwrap();
        child.kill().unwrap();
        child.wait().unwrap();
    }
    fn record(&self) -> Vec<u8> {
        fs::read(self.path().join("software-fan-safety.json")).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        if std::thread::panicking() {
            eprintln!(
                "保留失败日志：{}",
                self.dir.take().unwrap().keep().display()
            );
        }
    }
}

#[test]
fn killed_control_process_leaves_a_restart_block_until_an_explicit_successful_confirmation() {
    let mut f = Fixture::new();
    assert!(f.succeeds("check"));
    f.kill_running_writer();
    let before = f.record();
    assert!(!f.succeeds("check"));
    assert!(!f.succeeds("check"));
    assert_eq!(f.record(), before);
    assert!(f.succeeds("confirmed"));
    assert!(f.succeeds("check"));
}

#[test]
fn faulted_stops_stay_blocked_across_processes_but_normal_stops_do_not() {
    let f = Fixture::new();
    assert!(f.succeeds("clean"));
    assert!(f.succeeds("check"));
    assert!(f.succeeds("fault"));
    assert!(!f.succeeds("check"));
    assert!(!f.succeeds("clean"));
    assert!(f.succeeds("confirmed"));
    assert!(f.succeeds("clean"));
    assert!(f.succeeds("check"));
}
