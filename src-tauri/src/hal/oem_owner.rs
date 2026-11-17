use anyhow::{Result, bail};
use windows_sys::Win32::{
    Foundation::{CloseHandle, ERROR_NO_MORE_FILES, INVALID_HANDLE_VALUE},
    System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
        TH32CS_SNAPPROCESS,
    },
};
pub(crate) fn ensure_owner_absent() -> Result<()> {
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot == INVALID_HANDLE_VALUE {
        return Err(std::io::Error::last_os_error().into());
    }
    let result = inspect(snapshot);
    unsafe {
        CloseHandle(snapshot);
    };
    result
}
fn inspect(snapshot: windows_sys::Win32::Foundation::HANDLE) -> Result<()> {
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
        if [
            "FnKey.exe",
            "LaunchFlexikey.exe",
            "Flexikey.exe",
            "CC40.exe",
            "CC.exe",
        ]
        .iter()
        .any(|owner| name.eq_ignore_ascii_case(owner))
        {
            bail!(
                "原厂{name}仍可能处理原厂按键或修改设置，请先关闭后再预览。本应用不会结束原厂进程"
            );
        }
        found = unsafe { Process32NextW(snapshot, &mut entry) };
    }
    let error = std::io::Error::last_os_error();
    if error.raw_os_error() != Some(ERROR_NO_MORE_FILES as i32) {
        return Err(error.into());
    }
    Ok(())
}
