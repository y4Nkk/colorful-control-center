#[path = "../../../src-tauri/src/shutdown/deadline.rs"]
mod deadline;
#[path = "../../../src-tauri/src/shutdown/lifecycle.rs"]
mod lifecycle;
#[path = "../../../src-tauri/src/tray/native.rs"]
mod native;

use lifecycle::Lifecycle;
use native::{Action, Tray};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
    sync::OnceLock,
    thread,
    time::{Duration, Instant},
};
use windows_sys::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_ENDSESSION};

static LIFECYCLE: Lifecycle = Lifecycle::new();
static DEADLINE: OnceLock<deadline::Deadline> = OnceLock::new();
const BUDGET: Duration = Duration::from_millis(1200);

fn wait_forever() -> ! {
    loop {
        thread::park();
    }
}

#[test]
#[ignore = "仅由隔离进程测试启动"]
fn deadline_worker() {
    deadline::run_child().unwrap();
}

fn request(budget: Duration) {
    LIFECYCLE.request(|| deadline::arm_or_terminate(DEADLINE.get(), budget));
}

#[test]
#[ignore = "仅由隔离进程测试启动"]
fn exit_fixture() {
    assert_eq!(deadline::FLAG, "--exit-deadline");
    let mode = std::env::var_os("CCC_EXIT_FIXTURE_MODE").unwrap();
    let output = PathBuf::from(std::env::var_os("CCC_EXIT_FIXTURE_OUTPUT").unwrap());
    if mode == "missing-watchdog" {
        request(BUDGET);
        panic!("没有监护时应结束请求退出的进程");
    }
    let mut command = Command::new(std::env::current_exe().unwrap());
    command.args(["--ignored", "--exact", "deadline_worker"]);
    assert!(
        DEADLINE
            .set(deadline::Deadline::launch(command).unwrap())
            .is_ok()
    );
    match mode.to_str().unwrap() {
        "commit" => commit_then_exit(&output),
        "blocked-operation" => {
            let _operation = LIFECYCLE.enter().unwrap();
            request(BUDGET);
            assert!(LIFECYCLE.enter().is_none());
            request(Duration::from_secs(15));
            LIFECYCLE.drain();
        }
        "blocked-cleanup" => {
            request(BUDGET);
            LIFECYCLE.drain();
            fs::write(output.join("cleanup-started"), []).unwrap();
            wait_forever();
        }
        "native-close" => native_close(&output, false),
        "blocked-main" => native_close(&output, true),
        unknown => panic!("未知隔离模式 {unknown}"),
    }
}

fn commit_then_exit(output: &Path) {
    let operation = LIFECYCLE.enter().unwrap();
    let hardware = std::sync::Mutex::new(());
    {
        let _write = hardware.lock().unwrap();
        fs::write(output.join("applied"), []).unwrap();
    }
    thread::scope(|scope| {
        scope.spawn(move || {
            LIFECYCLE.wait_for_request();
            thread::sleep(Duration::from_millis(80));
            fs::write(output.join("commit-ack"), []).unwrap();
            drop(operation);
        });
        request(BUDGET);
        assert!(LIFECYCLE.enter().is_none());
        LIFECYCLE.drain();
        assert!(output.join("commit-ack").exists());
        fs::write(output.join("clean"), []).unwrap();
        LIFECYCLE.complete(0);
        std::process::exit(LIFECYCLE.wait_for_completion());
    });
}

fn native_close(output: &Path, block_main: bool) {
    let tray = Tray::start(
        None,
        |action| match action {
            Action::Quit => request(BUDGET),
            Action::Profile(name) => panic!("夹具意外收到档案 {name}"),
            _ => panic!("退出夹具收到其他托盘操作"),
        },
        || LIFECYCLE.closing(),
    )
    .unwrap();
    tray.set_profiles(Some(vec!["隔离档案".into()]));
    assert_ne!(
        unsafe { PostMessageW(tray.window(), WM_ENDSESSION, 1, 0) },
        0
    );
    thread::scope(|scope| {
        scope.spawn(move || {
            LIFECYCLE.wait_for_request();
            LIFECYCLE.drain();
            tray.stop().unwrap();
            fs::write(output.join("clean"), []).unwrap();
            LIFECYCLE.complete(0);
        });
        if block_main {
            wait_forever();
        }
        std::process::exit(LIFECYCLE.wait_for_completion());
    });
}

fn fixture(mode: &str, output: &Path) -> ExitStatus {
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", "exit_fixture"])
        .env("CCC_EXIT_FIXTURE_MODE", mode)
        .env("CCC_EXIT_FIXTURE_OUTPUT", output)
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return status;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("隔离进程 {mode} 未退出");
        }
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn waits_for_commit_ack_after_hardware_lock_is_released() {
    let dir = tempfile::tempdir().unwrap();
    assert!(fixture("commit", dir.path()).success());
    for marker in ["applied", "commit-ack", "clean"] {
        assert!(dir.path().join(marker).exists());
    }
}

#[test]
fn blocked_operation_and_repeated_requests_respect_first_deadline() {
    let dir = tempfile::tempdir().unwrap();
    let status = fixture("blocked-operation", dir.path());
    assert_eq!(status.code(), Some(deadline::TIMEOUT_EXIT_CODE as i32));
}

#[test]
fn stalled_cleanup_cannot_hold_process_open() {
    let dir = tempfile::tempdir().unwrap();
    let status = fixture("blocked-cleanup", dir.path());
    assert!(dir.path().join("cleanup-started").exists());
    assert_eq!(status.code(), Some(deadline::TIMEOUT_EXIT_CODE as i32));
}

#[test]
fn missing_watchdog_cutoff_finishes_the_requesting_process() {
    let dir = tempfile::tempdir().unwrap();
    let status = fixture("missing-watchdog", dir.path());
    assert_eq!(status.code(), Some(deadline::TIMEOUT_EXIT_CODE as i32));
}

#[test]
fn native_tray_window_can_request_session_exit_and_remove_itself() {
    let dir = tempfile::tempdir().unwrap();
    assert!(fixture("native-close", dir.path()).success());
    assert!(dir.path().join("clean").exists());
}

#[test]
fn independent_native_thread_starts_exit_with_a_blocked_main_thread() {
    let dir = tempfile::tempdir().unwrap();
    let status = fixture("blocked-main", dir.path());
    assert!(dir.path().join("clean").exists());
    assert_eq!(status.code(), Some(deadline::TIMEOUT_EXIT_CODE as i32));
}

#[test]
fn lifecycle_rejects_new_operations_and_returns_completed_code() {
    let local = Lifecycle::new();
    assert_eq!(local.exit_code(), None);
    local.request(|| {});
    assert!(local.closing());
    assert!(local.enter().is_none());
    local.complete(1);
    assert_eq!(local.exit_code(), Some(1));
}
