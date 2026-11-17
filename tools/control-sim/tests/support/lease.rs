use std::{
    collections::BTreeMap,
    fs,
    os::windows::{
        io::{AsRawHandle, FromRawHandle, OwnedHandle},
        process::CommandExt,
    },
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use windows_sys::Win32::System::Threading::{
    OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE, SYNCHRONIZATION_SYNCHRONIZE,
    TerminateProcess, WaitForSingleObject,
};

pub struct Fixture {
    pub name: String,
    root: PathBuf,
    dir: PathBuf,
    children: BTreeMap<String, Child>,
    guard: Option<OwnedHandle>,
}

impl Fixture {
    pub fn new() -> Self {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/lease-tests");
        fs::create_dir_all(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let id = format!("{}-{nonce}", std::process::id());
        let dir = root.join(&id);
        fs::create_dir(&dir).unwrap();
        Self {
            name: format!("Global\\ccc-lease-fixture-{id}"),
            root,
            dir,
            children: BTreeMap::new(),
            guard: None,
        }
    }
    pub fn spawn(&mut self, label: &str, scenario: &str) {
        assert!(!self.children.contains_key(label));
        let child = Command::new(env!("CARGO_BIN_EXE_lease-fixture"))
            .args([
                self.dir.as_os_str(),
                scenario.as_ref(),
                self.name.as_ref(),
                label.as_ref(),
            ])
            .creation_flags(0x08000000)
            .stdin(Stdio::null())
            .stdout(fs::File::create(self.dir.join(format!("{label}.stdout"))).unwrap())
            .stderr(fs::File::create(self.dir.join(format!("{label}.stderr"))).unwrap())
            .spawn()
            .unwrap();
        self.children.insert(label.into(), child);
    }
    pub fn wait(&self, name: &str) -> String {
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            match fs::read_to_string(self.dir.join(name)) {
                Ok(value) if !value.is_empty() => return value,
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => panic!("{e}"),
                _ => (),
            }
            assert!(
                Instant::now() < deadline,
                "等待 {name} 超时：{}",
                self.dir.display()
            );
            std::thread::sleep(Duration::from_millis(15));
        }
    }
    pub fn exists(&self, name: &str) -> bool {
        self.dir.join(name).exists()
    }
    pub fn signal(&self, name: &str) {
        fs::write(self.dir.join(name), "1").unwrap();
    }
    pub fn finish(&mut self, label: &str, success: bool) {
        let child = self.children.get_mut(label).unwrap();
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                assert_eq!(
                    status.success(),
                    success,
                    "{}：{status}",
                    self.dir.display()
                );
                return;
            }
            assert!(Instant::now() < deadline, "进程 {label} 未结束");
            std::thread::sleep(Duration::from_millis(15));
        }
    }
    pub fn kill(&mut self, label: &str) {
        let child = self.children.get_mut(label).unwrap();
        child.kill().unwrap();
        child.wait().unwrap();
    }
    pub fn retain_guard_handle(&mut self, label: &str) {
        assert!(self.guard.is_none());
        let pid = self.wait(&format!("{label}.child.pid")).parse().unwrap();
        let handle = unsafe {
            OpenProcess(
                PROCESS_TERMINATE | PROCESS_QUERY_LIMITED_INFORMATION | SYNCHRONIZATION_SYNCHRONIZE,
                0,
                pid,
            )
        };
        assert!(!handle.is_null());
        self.guard = Some(unsafe { OwnedHandle::from_raw_handle(handle) });
    }
    pub fn kill_guard(&self) {
        let handle = self.guard.as_ref().unwrap().as_raw_handle();
        assert_ne!(unsafe { TerminateProcess(handle, 23) }, 0);
        assert_eq!(unsafe { WaitForSingleObject(handle, 3000) }, 0);
    }
    pub fn wait_guard_exit(&self) {
        let handle = self.guard.as_ref().unwrap().as_raw_handle();
        assert_eq!(unsafe { WaitForSingleObject(handle, 8000) }, 0);
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        for child in self.children.values_mut() {
            if child.try_wait().ok().flatten().is_none() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
        if let Some(handle) = &self.guard {
            let handle = handle.as_raw_handle();
            if unsafe { WaitForSingleObject(handle, 0) } != 0 {
                unsafe {
                    TerminateProcess(handle, 23);
                    WaitForSingleObject(handle, 3000);
                }
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
