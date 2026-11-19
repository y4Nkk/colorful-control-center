mod fn_key;

use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};

use anyhow::{Result, anyhow};
use windows_sys::Win32::{
    Foundation::{ERROR_NO_MORE_FILES, HANDLE, INVALID_HANDLE_VALUE},
    System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
        TH32CS_SNAPPROCESS,
    },
};

pub(crate) use fn_key::{FnKeyIdentity, FnKeyProcess};

const FN_KEY: &str = "FnKey.exe";
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
        anyhow!(
            "原厂 {} 正在运行，可能同时修改硬件设置。请退出后重试。",
            self.executable
        )
    }
}

pub(crate) fn ensure_owner_absent() -> Result<()> {
    match owners()?.first() {
        Some(owner) => Err(owner.blocked()),
        None => Ok(()),
    }
}

fn owners() -> Result<Vec<Owner>> {
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot == INVALID_HANDLE_VALUE {
        return Err(std::io::Error::last_os_error().into());
    }
    let snapshot = unsafe { OwnedHandle::from_raw_handle(snapshot) };
    inspect(snapshot.as_raw_handle())
}

fn inspect(snapshot: HANDLE) -> Result<Vec<Owner>> {
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
        if let Some(executable) = EXECUTABLES
            .into_iter()
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
