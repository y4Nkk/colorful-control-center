mod fn_key;

use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};

use anyhow::{Context, Result, anyhow, ensure};
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
const EXECUTABLES: [&str; 5] = [
    FN_KEY,
    "LaunchFlexikey.exe",
    "Flexikey.exe",
    "CC40.exe",
    "CC.exe",
];

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
    EXECUTABLES
        .iter()
        .any(|executable| blocked_message(executable) == reason)
}

pub(crate) fn ensure_owner_absent() -> Result<()> {
    ensure_absent(&EXECUTABLES)
}

pub(crate) fn close_at_startup() -> Result<bool> {
    let active = owners(&EXECUTABLES)?;
    let mut fn_keys = active.iter().filter(|owner| owner.executable == FN_KEY);
    if let Some(owner) = fn_keys.next() {
        ensure!(
            fn_keys.next().is_none(),
            "检测到多个 FnKey.exe，请手动退出原厂软件后重试"
        );
        fn_key::terminate_verified(owner.pid).context("结束原厂 FnKey 失败")?;
        ensure_owner_absent().context("已结束原厂 FnKey，复查原厂控制进程失败")?;
        return Ok(true);
    }
    if let Some(owner) = active.first() {
        return Err(owner.blocked());
    }
    Ok(false)
}

pub(crate) fn blocked_reason() -> Option<String> {
    ensure_owner_absent()
        .err()
        .map(|error| format!("{error:#}"))
}

pub(crate) fn ensure_battery_owner_absent() -> Result<()> {
    ensure_absent(&BATTERY_EXECUTABLES)
}

fn ensure_absent(executables: &[&'static str]) -> Result<()> {
    match owners(executables)?.first() {
        Some(owner) => Err(owner.blocked()),
        None => Ok(()),
    }
}

fn owners(executables: &[&'static str]) -> Result<Vec<Owner>> {
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot == INVALID_HANDLE_VALUE {
        return Err(std::io::Error::last_os_error().into());
    }
    let snapshot = unsafe { OwnedHandle::from_raw_handle(snapshot) };
    inspect(snapshot.as_raw_handle(), executables)
}

fn inspect(snapshot: HANDLE, executables: &[&'static str]) -> Result<Vec<Owner>> {
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
        let name = String::from_utf16(&entry.szExeFile[..end])?;
        if let Some(executable) = executables
            .iter()
            .copied()
            .find(|owner| name.eq_ignore_ascii_case(owner))
        {
            owners.push(Owner {
                executable,
                pid: entry.th32ProcessID,
            });
        }
        found = unsafe { Process32NextW(snapshot, &mut entry) };
    }
    let error = std::io::Error::last_os_error();
    if error.raw_os_error() != Some(ERROR_NO_MORE_FILES as i32) {
        return Err(error.into());
    }
    Ok(owners)
}
