//! 只读识别已启动的固件 DTT 设备或原厂的驱动安装条件。
use anyhow::{Result, bail};
use windows_sys::Win32::{
    Devices::{
        DeviceAndDriverInstallation::*,
        Properties::{DEVPKEY_Device_BiosDeviceName, DEVPROP_TYPE_STRING},
    },
    System::Registry::*,
};
fn matches(name: &str) -> bool {
    name.to_ascii_lowercase().contains("dynamic tuning")
}
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}
struct Devices(HDEVINFO);
impl Drop for Devices {
    fn drop(&mut self) {
        unsafe {
            SetupDiDestroyDeviceInfoList(self.0);
        }
    }
}
fn find_device(qualifies: impl Fn(HDEVINFO, &SP_DEVINFO_DATA) -> Result<bool>) -> Result<bool> {
    let handle = unsafe {
        SetupDiGetClassDevsW(
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null_mut(),
            DIGCF_ALLCLASSES | DIGCF_PRESENT,
        )
    };
    if handle == -1isize {
        return Err(std::io::Error::last_os_error().into());
    }
    let set = Devices(handle);
    for index in 0..u32::MAX {
        let mut info: SP_DEVINFO_DATA = unsafe { std::mem::zeroed() };
        info.cbSize = size_of_val(&info) as u32;
        if unsafe { SetupDiEnumDeviceInfo(set.0, index, &mut info) } == 0 {
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() == Some(259) {
                return Ok(false);
            }
            return Err(error.into());
        }
        if qualifies(set.0, &info)? {
            return Ok(true);
        }
    }
    bail!("DTT设备枚举未正常结束")
}
fn dtt_description(set: HDEVINFO, info: &SP_DEVINFO_DATA) -> Result<bool> {
    let mut text = [0u16; 4096];
    if unsafe {
        SetupDiGetDeviceRegistryPropertyW(
            set,
            info,
            SPDRP_DEVICEDESC,
            std::ptr::null_mut(),
            text.as_mut_ptr().cast(),
            size_of_val(&text) as u32,
            std::ptr::null_mut(),
        )
    } == 0
    {
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() == Some(13) {
            return Ok(false);
        }
        return Err(error.into());
    }
    let end = text.iter().position(|v| *v == 0).unwrap_or(text.len());
    Ok(matches(&String::from_utf16_lossy(&text[..end])))
}

pub(super) fn active_platform() -> Result<bool> {
    find_device(|_, info| {
        let mut name = [0u16; 512];
        let (mut kind, mut length) = (0, size_of_val(&name) as u32);
        let result = unsafe {
            CM_Get_DevNode_PropertyW(
                info.DevInst,
                &DEVPKEY_Device_BiosDeviceName,
                &mut kind,
                name.as_mut_ptr().cast(),
                &mut length,
                0,
            )
        };
        match result {
            CR_SUCCESS => (),
            CR_NO_SUCH_VALUE | CR_NO_SUCH_DEVNODE | CR_INVALID_DEVNODE => return Ok(false),
            _ => bail!("读取 ACPI 设备路径失败，CONFIGRET={result:#x}"),
        }
        let end = name
            .iter()
            .position(|value| *value == 0)
            .unwrap_or(name.len());
        let path = String::from_utf16_lossy(&name[..end]);
        if kind != DEVPROP_TYPE_STRING
            || !path
                .trim_start_matches('\\')
                .eq_ignore_ascii_case("_SB.IETM")
        {
            return Ok(false);
        }
        // DptfTabl exposes IETM only when DPTF is enabled on the reviewed platform.
        let (mut status, mut problem) = (0, 0);
        let result = unsafe { CM_Get_DevNode_Status(&mut status, &mut problem, info.DevInst, 0) };
        match result {
            CR_SUCCESS => {
                let active = DN_STARTED | DN_DRIVER_LOADED;
                Ok(problem == 0 && status & active == active)
            }
            CR_NO_SUCH_DEVNODE | CR_INVALID_DEVNODE => Ok(false),
            _ => bail!("读取 IETM 设备状态失败，CONFIGRET={result:#x}"),
        }
    })
}

struct Key(HKEY);
impl Drop for Key {
    fn drop(&mut self) {
        unsafe {
            RegCloseKey(self.0);
        }
    }
}
fn open(root: HKEY, path: &str, view: u32) -> Result<Option<Key>> {
    let mut handle = std::ptr::null_mut();
    let code = unsafe { RegOpenKeyExW(root, wide(path).as_ptr(), 0, KEY_READ | view, &mut handle) };
    match code {
        0 => Ok(Some(Key(handle))),
        2 => Ok(None),
        _ => Err(std::io::Error::from_raw_os_error(code as i32).into()),
    }
}
fn display_name(key: &Key) -> Result<Option<String>> {
    let mut text = [0u16; 4096];
    let mut bytes = size_of_val(&text) as u32;
    let mut kind = 0;
    let code = unsafe {
        RegQueryValueExW(
            key.0,
            wide("DisplayName").as_ptr(),
            std::ptr::null(),
            &mut kind,
            text.as_mut_ptr().cast(),
            &mut bytes,
        )
    };
    if code == 2 {
        return Ok(None);
    }
    if code != 0 {
        return Err(std::io::Error::from_raw_os_error(code as i32).into());
    }
    if kind != REG_SZ && kind != REG_EXPAND_SZ {
        return Ok(None);
    }
    let length = (bytes as usize / 2).min(text.len());
    Ok(Some(
        String::from_utf16_lossy(&text[..length])
            .trim_end_matches('\0')
            .to_owned(),
    ))
}
fn uninstall(root: HKEY, view: u32) -> Result<bool> {
    let Some(key) = open(
        root,
        "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall",
        view,
    )?
    else {
        return Ok(false);
    };
    for index in 0..u32::MAX {
        let mut name = [0u16; 256];
        let mut length = name.len() as u32;
        let code = unsafe {
            RegEnumKeyExW(
                key.0,
                index,
                name.as_mut_ptr(),
                &mut length,
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        if code == 259 {
            return Ok(false);
        }
        if code != 0 {
            return Err(std::io::Error::from_raw_os_error(code as i32).into());
        }
        if let Some(item) = open(
            key.0,
            &String::from_utf16_lossy(&name[..length as usize]),
            0,
        )? {
            if display_name(&item)?.is_some_and(|name| matches(&name)) {
                return Ok(true);
            }
        }
    }
    bail!("DTT安装项枚举未正常结束")
}
pub(super) fn installed() -> Result<bool> {
    let mut errors = Vec::new();
    for result in [
        find_device(dtt_description),
        uninstall(HKEY_LOCAL_MACHINE, KEY_WOW64_64KEY),
        uninstall(HKEY_LOCAL_MACHINE, KEY_WOW64_32KEY),
        uninstall(HKEY_CURRENT_USER, KEY_WOW64_64KEY),
    ] {
        match result {
            Ok(true) => return Ok(true),
            Ok(false) => {}
            Err(e) => errors.push(format!("{e:#}")),
        }
    }
    if !errors.is_empty() {
        bail!(errors.join("；"));
    }
    Ok(false)
}
#[cfg(test)]
mod tests {
    #[test]
    fn original_driver_description_is_required() {
        assert!(super::matches("Intel(R) Dynamic Tuning Technology"));
        assert!(super::matches("DYNAMIC TUNING"));
        assert!(!super::matches("Intel graphics driver"));
        assert!(!super::matches("Innovation Platform Framework"));
    }
}
