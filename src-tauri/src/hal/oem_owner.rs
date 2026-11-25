mod fn_key;
mod process;

use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow};
use windows_sys::Win32::{
    Foundation::{ERROR_NO_MORE_FILES, HANDLE, INVALID_HANDLE_VALUE},
    System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
        TH32CS_SNAPPROCESS,
    },
};

pub(crate) use fn_key::{FnKeyIdentity, FnKeyProcess};

const FN_KEY: &str = "FnKey.exe";
const BATTERY_EXECUTABLES: [&str; 2] = ["BatteryPackUtility.exe", FN_KEY];

fn control_executables() -> impl Iterator<Item = &'static str> {
    process::TARGETS
        .iter()
        .map(|target| target.executable)
        .chain(["Flexikey.exe"])
}

#[derive(Default)]
pub(crate) struct CloseReport {
    pub closed: Vec<&'static str>,
    pub failures: Vec<String>,
}

struct Owner {
    executable: &'static str,
    pid: u32,
}

impl Owner {
    fn blocked(&self) -> anyhow::Error {
        anyhow!(blocked_message(self.executable))
    }
}

fn blocked_message(executable: &str) -> String {
    format!("原厂 {executable} 正在运行，可能同时修改硬件设置。请退出后重试。")
}

pub(crate) fn is_blocked_reason(reason: &str) -> bool {
    control_executables().any(|executable| blocked_message(executable) == reason)
}

pub(crate) fn ensure_owner_absent() -> Result<()> {
    ensure_absent(control_owners()?)
}

pub(crate) fn close_running() -> Result<CloseReport> {
    let mut report = CloseReport::default();
    let mut pending = Vec::new();
    for owner in control_owners()? {
        let Some(target) = process::TARGETS
            .iter()
            .find(|target| target.executable == owner.executable)
        else {
            report.failures.push(blocked_message(owner.executable));
            continue;
        };
        match process::request_exit(owner.pid, target) {
            Ok(Some(handle)) => pending.push((target.executable, handle)),
            Ok(None) => (),
            Err(error) => report
                .failures
                .push(format!("关闭 {} 失败：{error:#}", target.executable)),
        }
    }
    let deadline = Instant::now() + Duration::from_secs(2);
    for (executable, handle) in pending {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match process::wait_for_exit(&handle, remaining.as_millis() as u32) {
            Ok(true) => report.closed.push(executable),
            Ok(false) => report.failures.push(format!("等待 {executable} 退出超时")),
            Err(error) => report
                .failures
                .push(format!("确认 {executable} 退出失败：{error:#}")),
        }
    }
    report.closed.sort_unstable();
    report.closed.dedup();
    report.failures.sort_unstable();
    report.failures.dedup();
    Ok(report)
}

pub(crate) fn blocked_reason() -> Option<String> {
    ensure_owner_absent()
        .err()
        .map(|error| format!("{error:#}"))
}

pub(crate) fn ensure_battery_owner_absent() -> Result<()> {
    ensure_absent(owners(|name| {
        matching_executable(name, BATTERY_EXECUTABLES.into_iter())
    })?)
}

fn ensure_absent(owners: Vec<Owner>) -> Result<()> {
    match owners.first() {
        Some(owner) => Err(owner.blocked()),
        None => Ok(()),
    }
}

fn control_owners() -> Result<Vec<Owner>> {
    owners(|name| matching_executable(name, control_executables()))
}

fn matching_executable(
    name: &str,
    mut candidates: impl Iterator<Item = &'static str>,
) -> Option<&'static str> {
    candidates.find(|candidate| name.eq_ignore_ascii_case(candidate))
}

fn owners(recognize: impl Fn(&str) -> Option<&'static str>) -> Result<Vec<Owner>> {
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot == INVALID_HANDLE_VALUE {
        return Err(std::io::Error::last_os_error().into());
    }
    let snapshot = unsafe { OwnedHandle::from_raw_handle(snapshot) };
    inspect(snapshot.as_raw_handle(), recognize)
}

fn inspect(
    snapshot: HANDLE,
    recognize: impl Fn(&str) -> Option<&'static str>,
) -> Result<Vec<Owner>> {
    let mut owners = Vec::new();
    let mut entry = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    let mut found = unsafe { Process32FirstW(snapshot, &mut entry) };
    while found != 0 {
        let end = entry
            .szExeFile
            .iter()
            .position(|v| *v == 0)
            .unwrap_or(entry.szExeFile.len());
        let name = String::from_utf16_lossy(&entry.szExeFile[..end]);
        if let Some(executable) = recognize(&name) {
            owners.push(Owner {
                executable,
                pid: entry.th32ProcessID,
            });
        }
        found = unsafe { Process32NextW(snapshot, &mut entry) };
    }
    let error = std::io::Error::last_os_error();
    if error.raw_os_error() != Some(ERROR_NO_MORE_FILES as i32) {
        return Err(error).context("无法枚举 Windows 进程");
    }
    Ok(owners)
}
