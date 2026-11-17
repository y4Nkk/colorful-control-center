//! 只运行项目自带假进程；系统屏保、OEM程序及硬件均不参与。
#[path = "../../../src-tauri/src/owned_process.rs"]
mod owned_process;

use owned_process::Process;
use std::{
    ffi::OsString,
    os::windows::{
        ffi::OsStringExt,
        io::{AsRawHandle, FromRawHandle, OwnedHandle},
        process::CommandExt,
    },
    path::{Path, PathBuf},
    process::{Child, Command},
    thread,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::{WAIT_OBJECT_0, WAIT_TIMEOUT},
    System::Threading::{
        CREATE_NO_WINDOW, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
        WaitForSingleObject,
    },
};

fn executable() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_owned-process-fixture"))
}
fn args(dir: &Path, extra: &[&str]) -> Vec<OsString> {
    let mut values = vec![
        "child".into(),
        dir.join("ready").into(),
        dir.join("finish").into(),
    ];
    values.extend(extra.iter().map(OsString::from));
    values
}
fn read(path: &Path) -> String {
    let end = Instant::now() + Duration::from_secs(10);
    loop {
        if let Ok(value) = std::fs::read_to_string(path) {
            if !value.is_empty() {
                return value;
            }
        }
        assert!(Instant::now() < end, "等待隔离输出超时：{}", path.display());
        thread::sleep(Duration::from_millis(10));
    }
}
fn watch(pid: u32) -> OwnedHandle {
    let handle = unsafe {
        OpenProcess(
            PROCESS_SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION,
            0,
            pid,
        )
    };
    assert!(
        !handle.is_null(),
        "无法取得测试进程句柄：{}",
        std::io::Error::last_os_error()
    );
    unsafe { OwnedHandle::from_raw_handle(handle) }
}
fn status(handle: &OwnedHandle, timeout: u32) -> u32 {
    unsafe { WaitForSingleObject(handle.as_raw_handle(), timeout) }
}
struct Background(Child);
impl Drop for Background {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}
fn spawn_owner(dir: &Path) -> Background {
    Background(
        Command::new(executable())
            .creation_flags(CREATE_NO_WINDOW)
            .arg("owner")
            .arg(dir)
            .spawn()
            .unwrap(),
    )
}
fn unrelated(dir: &Path) -> Background {
    Background(
        Command::new(executable())
            .creation_flags(CREATE_NO_WINDOW)
            .args(args(dir, &[]))
            .spawn()
            .unwrap(),
    )
}
fn ready(path: &Path) -> (u32, Vec<String>) {
    serde_json::from_str(&read(path)).unwrap()
}

#[test]
fn prepared_child_is_suspended_and_dropping_owner_kills_only_its_own_child() {
    let own = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    let mut outsider = unrelated(other.path());
    let _ = ready(&other.path().join("ready"));
    let process = Process::prepare(&executable(), &args(own.path(), &[])).unwrap();
    let handle = watch(process.id());
    assert_eq!(status(&handle, 100), WAIT_TIMEOUT);
    assert!(!own.path().join("ready").exists());
    drop(process);
    assert_eq!(status(&handle, 5000), WAIT_OBJECT_0);
    assert!(outsider.0.try_wait().unwrap().is_none());
}
#[test]
fn started_child_receives_literal_arguments_and_stop_preserves_unrelated_process() {
    let own = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    let mut outsider = unrelated(other.path());
    let _ = ready(&other.path().join("ready"));
    let values = [
        "a b",
        "中文",
        "",
        "quoted \"word\"",
        r"slash\\",
        r#"a\\\"b"#,
        "$(literal)",
    ];
    let mut process = Process::prepare(&executable(), &args(own.path(), &values)).unwrap();
    let handle = watch(process.id());
    process.start().unwrap();
    let (pid, actual) = ready(&own.path().join("ready"));
    assert_eq!(pid, process.id());
    assert_eq!(actual, values);
    assert!(process.start().is_err());
    assert!(process.poll().unwrap().is_none());
    process.stop().unwrap();
    assert_eq!(status(&handle, 5000), WAIT_OBJECT_0);
    assert!(outsider.0.try_wait().unwrap().is_none());
}
#[test]
fn killed_owner_closes_its_job_before_and_after_child_activation() {
    for activated in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let mut owner = spawn_owner(dir.path());
        let pid: u32 = read(&dir.path().join("child-pid")).parse().unwrap();
        let handle = watch(pid);
        if activated {
            std::fs::write(dir.path().join("activate"), "go").unwrap();
            let (reported, _) = ready(&dir.path().join("child-ready"));
            assert_eq!(reported, pid);
        } else {
            assert!(!dir.path().join("child-ready").exists());
        }
        owner.0.kill().unwrap();
        owner.0.wait().unwrap();
        assert_eq!(status(&handle, 5000), WAIT_OBJECT_0);
    }
}
#[test]
fn stopping_job_terminates_descendant_even_after_direct_child_exits() {
    let dir = tempfile::tempdir().unwrap();
    let mut process = Process::prepare(
        &executable(),
        &["tree".into(), dir.path().as_os_str().into()],
    )
    .unwrap();
    process.start().unwrap();
    let (pid, _) = ready(&dir.path().join("leaf-ready"));
    let leaf = watch(pid);
    let end = Instant::now() + Duration::from_secs(10);
    while process.poll().unwrap().is_none() {
        assert!(Instant::now() < end);
        thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(status(&leaf, 0), WAIT_TIMEOUT);
    process.stop().unwrap();
    assert_eq!(status(&leaf, 5000), WAIT_OBJECT_0);
}
#[test]
fn relative_executable_and_nul_arguments_are_rejected_before_child_creation() {
    assert!(Process::prepare(Path::new("relative.exe"), &[]).is_err());
    assert!(Process::prepare(&executable(), &[OsString::from_wide(&[65, 0, 66])]).is_err());
}
