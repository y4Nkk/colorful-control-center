use std::{
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    path::Path,
    ptr::null_mut,
};

use anyhow::{Context, Result, ensure};
use windows_sys::Win32::{
    Foundation::{ERROR_INSUFFICIENT_BUFFER, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT},
    Storage::Packaging::Appx::{
        GetApplicationUserModelId, GetPackageFamilyName, GetPackageFullName,
        GetPackagePathByFullName,
    },
    System::{
        RemoteDesktop::ProcessIdToSessionId,
        Threading::{
            OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
            PROCESS_SYNCHRONIZE, PROCESS_TERMINATE, QueryFullProcessImageNameW, TerminateProcess,
            WaitForSingleObject,
        },
    },
};

pub(super) struct Target {
    pub executable: &'static str,
    family: &'static str,
    directory: &'static str,
    main_application: bool,
}

const FN_KEY_FAMILY: &str = "CLEVOCO.FnhotkeysandOSD_6h6z29zh29qx0";
const FLEXIKEY_FAMILY: &str = "CLEVOCO.Flexikey_6h6z29zh29qx0";
pub(super) const FN_KEY_TARGET: Target = Target {
    executable: super::FN_KEY,
    family: FN_KEY_FAMILY,
    directory: "FnKey",
    main_application: true,
};

pub(super) const TARGETS: [Target; 5] = [
    FN_KEY_TARGET,
    Target {
        executable: "ControlCenter30.exe",
        family: "CLEVOCO.ControlCenter3.0_6h6z29zh29qx0",
        directory: "ControlCenter30",
        main_application: true,
    },
    Target {
        executable: "CC40.exe",
        family: FN_KEY_FAMILY,
        directory: r"FnKey\CC40",
        main_application: false,
    },
    Target {
        executable: "LaunchFlexikey.exe",
        family: FLEXIKEY_FAMILY,
        directory: "LaunchFlexikey",
        main_application: true,
    },
    Target {
        executable: "CC.exe",
        family: FLEXIKEY_FAMILY,
        directory: "LaunchFlexikey",
        main_application: false,
    },
];

pub(super) fn request_exit(pid: u32, target: &Target) -> Result<Option<OwnedHandle>> {
    let handle = open_verified(pid, target, PROCESS_TERMINATE)?;
    if wait_for_exit(&handle, 0)? {
        return Ok(None);
    }
    if unsafe { TerminateProcess(handle.as_raw_handle(), 1) } == 0 {
        let error = std::io::Error::last_os_error();
        if wait_for_exit(&handle, 0)? {
            return Ok(None);
        }
        return Err(error).context("未能结束进程");
    }
    Ok(Some(handle))
}

pub(super) fn open_verified(pid: u32, target: &Target, extra_access: u32) -> Result<OwnedHandle> {
    let handle = unsafe {
        OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE | extra_access,
            0,
            pid,
        )
    };
    if handle.is_null() {
        return Err(std::io::Error::last_os_error()).context("无法打开进程");
    }
    let handle = unsafe { OwnedHandle::from_raw_handle(handle) };
    ensure!(
        session_id(pid)? == session_id(std::process::id())?,
        "进程位于其他 Windows 会话"
    );
    verify_image(handle.as_raw_handle(), target)?;
    Ok(handle)
}

pub(super) fn wait_for_exit(handle: &OwnedHandle, milliseconds: u32) -> Result<bool> {
    match unsafe { WaitForSingleObject(handle.as_raw_handle(), milliseconds) } {
        WAIT_OBJECT_0 => Ok(true),
        WAIT_TIMEOUT => Ok(false),
        _ => Err(std::io::Error::last_os_error()).context("未能确认进程已退出"),
    }
}

fn session_id(pid: u32) -> Result<u32> {
    let mut session = 0;
    if unsafe { ProcessIdToSessionId(pid, &mut session) } == 0 {
        return Err(std::io::Error::last_os_error()).context("无法读取进程所属会话");
    }
    Ok(session)
}

fn verify_image(handle: HANDLE, target: &Target) -> Result<()> {
    let family =
        package_string(|length, text| unsafe { GetPackageFamilyName(handle, length, text) })
            .context("无法确认进程的原厂应用包")?;
    ensure!(
        family.eq_ignore_ascii_case(target.family),
        "进程不属于预期的原厂应用包"
    );
    if target.main_application {
        let app = package_string(|length, text| unsafe {
            GetApplicationUserModelId(handle, length, text)
        })
        .context("无法读取原厂应用 ID")?;
        ensure!(
            app.eq_ignore_ascii_case(&format!("{}!App", target.family)),
            "进程的应用 ID 与原厂应用不匹配"
        );
    }
    let name = package_string(|length, text| unsafe { GetPackageFullName(handle, length, text) })?;
    let name = name.encode_utf16().chain([0]).collect::<Vec<_>>();
    let directory = package_string(|length, text| unsafe {
        GetPackagePathByFullName(name.as_ptr(), length, text)
    })?;
    let expected = Path::new(&directory)
        .join(target.directory)
        .join(target.executable);
    let mut image = vec![0_u16; 32768];
    let mut length = image.len() as u32;
    if unsafe {
        QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, image.as_mut_ptr(), &mut length)
    } == 0
    {
        return Err(std::io::Error::last_os_error()).context("无法读取进程映像路径");
    }
    let image = String::from_utf16(&image[..length as usize])?;
    ensure!(
        image.eq_ignore_ascii_case(&expected.to_string_lossy()),
        "进程映像与应用包内路径不匹配"
    );
    Ok(())
}

fn package_string(read: impl Fn(*mut u32, *mut u16) -> u32) -> Result<String> {
    let mut length = 0;
    let code = read(&mut length, null_mut());
    if code != ERROR_INSUFFICIENT_BUFFER {
        return Err(std::io::Error::from_raw_os_error(code as i32).into());
    }
    let mut text = vec![0_u16; length as usize];
    let code = read(&mut length, text.as_mut_ptr());
    if code != 0 {
        return Err(std::io::Error::from_raw_os_error(code as i32).into());
    }
    Ok(String::from_utf16(
        text.split(|character| *character == 0)
            .next()
            .unwrap_or_default(),
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unrelated_process_cannot_be_verified_as_any_oem_target() {
        for target in &TARGETS {
            let error = open_verified(std::process::id(), target, 0).unwrap_err();
            let detail = format!("{error:#}");
            assert!(detail.contains("原厂应用包"), "{detail}");
        }
    }
}
