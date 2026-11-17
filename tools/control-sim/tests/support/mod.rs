use std::{
    fs,
    os::windows::process::CommandExt,
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
mod process;

pub struct Fixture {
    parent: Child,
    root: PathBuf,
    dir: PathBuf,
}

impl Fixture {
    pub fn start(scenario: &str) -> Self {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/process-tests");
        fs::create_dir_all(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = root.join(format!("{}-{nonce}-{scenario}", std::process::id()));
        fs::create_dir(&dir).unwrap();
        let parent = Command::new(env!("CARGO_BIN_EXE_watchdog-fixture"))
            .arg(&dir)
            .arg(scenario)
            .creation_flags(0x08000000)
            .stdin(Stdio::null())
            .stdout(fs::File::create(dir.join("stdout.log")).unwrap())
            .stderr(fs::File::create(dir.join("stderr.log")).unwrap())
            .spawn()
            .unwrap();
        Self { parent, root, dir }
    }

    pub fn exists(&self, name: &str) -> bool {
        self.dir.join(name).exists()
    }
    pub fn read(&self, name: &str) -> String {
        fs::read_to_string(self.dir.join(name)).unwrap()
    }
    pub fn release_output(&self) {
        fs::write(self.dir.join("release-output"), "1").unwrap();
    }

    pub fn wait_file(&self, name: &str) -> String {
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            match fs::read_to_string(self.dir.join(name)) {
                Ok(text) if !text.is_empty() => return text,
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                    panic!("{}: {e}", self.dir.display())
                }
                _ => (),
            }
            assert!(
                Instant::now() < deadline,
                "等待 {name} 超时；日志 {}",
                self.dir.display()
            );
            std::thread::sleep(Duration::from_millis(15));
        }
    }

    pub fn kill_parent(&mut self) {
        self.parent.kill().unwrap();
        self.parent.wait().unwrap();
    }

    pub fn wait_parent(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            if let Some(status) = self.parent.try_wait().unwrap() {
                assert!(status.success(), "父进程失败：{}", self.dir.display());
                return;
            }
            assert!(
                Instant::now() < deadline,
                "父进程未退出：{}",
                self.dir.display()
            );
            std::thread::sleep(Duration::from_millis(15));
        }
    }

    pub fn child_alive(&self) -> bool {
        let pid = self.read("child.pid").parse().unwrap();
        process::alive(pid)
    }

    pub fn wait_child_exit(&self) {
        let deadline = Instant::now() + Duration::from_secs(8);
        while self.child_alive() {
            assert!(
                Instant::now() < deadline,
                "子进程未退出：{}",
                self.dir.display()
            );
            std::thread::sleep(Duration::from_millis(15));
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if self.parent.try_wait().ok().flatten().is_none() {
            let _ = self.parent.kill();
            let _ = self.parent.wait();
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
