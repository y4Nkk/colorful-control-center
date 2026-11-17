//! 原厂FnKey在低电量事件59/60修改电池控制；同一电池只能有一个校准控制者。
use anyhow::{Result, bail};
use windows_sys::Win32::{
    Foundation::{CloseHandle, ERROR_NO_MORE_FILES, INVALID_HANDLE_VALUE},
    System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
        TH32CS_SNAPPROCESS,
    },
};

pub(crate) fn ensure_available() -> Result<()> {
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot == INVALID_HANDLE_VALUE {
        return Err(std::io::Error::last_os_error().into());
    }
    let result = inspect(snapshot);
    unsafe {
        CloseHandle(snapshot);
    }
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
        if name.eq_ignore_ascii_case("FnKey.exe")
            || name.eq_ignore_ascii_case("BatteryPackUtility.exe")
        {
            bail!("原厂 {name} 仍可能控制电池；请先关闭原厂控制程序。本应用不会结束原厂进程");
        }
        found = unsafe { Process32NextW(snapshot, &mut entry) };
    }
    let error = std::io::Error::last_os_error();
    if error.raw_os_error() != Some(ERROR_NO_MORE_FILES as i32) {
        return Err(error.into());
    }
    Ok(())
}
