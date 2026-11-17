//! 强杀的是本测试创建的专用文件写进程；不启动应用或访问用户档案。
use std::{
    fs,
    os::windows::process::CommandExt,
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

struct Fixture {
    root: PathBuf,
    dir: PathBuf,
    child: Option<Child>,
}

impl Fixture {
    fn new() -> Self {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/storage-tests");
        fs::create_dir_all(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = root.join(format!("中文 空间-{}-{nonce}", std::process::id()));
        fs::create_dir(&dir).unwrap();
        Self {
            root,
            dir,
            child: None,
        }
    }
    fn spawn(&mut self, mode: &str) {
        assert!(
            self.child
                .as_mut()
                .is_none_or(|child| child.try_wait().unwrap().is_some())
        );
        self.child = Some(
            Command::new(env!("CARGO_BIN_EXE_storage-fixture"))
                .arg(&self.dir)
                .arg(mode)
                .creation_flags(0x08000000)
                .stdin(Stdio::null())
                .stdout(fs::File::create(self.dir.join("stdout.log")).unwrap())
                .stderr(fs::File::create(self.dir.join("stderr.log")).unwrap())
                .spawn()
                .unwrap(),
        );
    }
    fn wait_staged(&self) -> PathBuf {
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            if let Ok(path) = fs::read_to_string(self.dir.join("staged")) {
                if !path.is_empty() {
                    let staged = PathBuf::from(path).canonicalize().unwrap();
                    assert_eq!(staged.parent(), Some(self.dir.as_path()));
                    assert_eq!(staged.extension().unwrap(), "tmp");
                    return staged;
                }
            }
            assert!(
                Instant::now() < deadline,
                "等待准备阶段超时：{}",
                self.dir.display()
            );
            std::thread::sleep(Duration::from_millis(15));
        }
    }
    fn kill(&mut self) {
        let child = self.child.as_mut().unwrap();
        child.kill().unwrap();
        child.wait().unwrap();
    }
    fn finish(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            if let Some(status) = self.child.as_mut().unwrap().try_wait().unwrap() {
                assert!(status.success(), "{}：{status}", self.dir.display());
                return;
            }
            assert!(Instant::now() < deadline, "进程未按期退出");
            std::thread::sleep(Duration::from_millis(15));
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if let Some(child) = &mut self.child {
            if child.try_wait().ok().flatten().is_none() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
        if std::thread::panicking() {
            eprintln!("保留失败日志：{}", self.dir.display());
            return;
        }
        let target = self.dir.canonicalize().unwrap();
        assert_eq!(target.parent(), Some(self.root.as_path()));
        fs::remove_dir_all(target).unwrap();
    }
}

#[test]
fn killed_writer_preserves_existing_profile_and_later_write_can_commit() {
    let mut f = Fixture::new();
    let target = f.dir.join("档案.json");
    let original = br#"{"profile":"original","pl1W":65}"#;
    fs::write(&target, original).unwrap();
    f.spawn("stage");
    let staged = f.wait_staged();
    for _ in 0..16 {
        assert_eq!(fs::read(&target).unwrap(), original);
    }
    f.kill();
    assert_eq!(fs::read(&target).unwrap(), original);
    assert!(staged.exists());
    f.spawn("commit");
    f.finish();
    let data: serde_json::Value = serde_json::from_slice(&fs::read(target).unwrap()).unwrap();
    assert_eq!(data["profile"], "new");
    assert_eq!(data["pl1W"], 75.125);
}

#[test]
fn killed_first_save_does_not_publish_a_partial_profile() {
    let mut f = Fixture::new();
    let target = f.dir.join("档案.json");
    f.spawn("stage");
    let staged = f.wait_staged();
    f.kill();
    assert!(!target.exists());
    assert!(staged.exists());
    f.spawn("commit");
    f.finish();
    let data: serde_json::Value = serde_json::from_slice(&fs::read(target).unwrap()).unwrap();
    assert_eq!(data["profile"], "new");
}
