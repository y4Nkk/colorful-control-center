use anyhow::{Result, anyhow, bail};
use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
use windows_sys::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ,
    RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
};

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const VALUE_NAME: &str = "ColorfulControlCenter";
struct Key(HKEY);
impl Drop for Key {
    fn drop(&mut self) {
        unsafe {
            RegCloseKey(self.0);
        }
    }
}
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain([0]).collect()
}

fn open(write: bool) -> Result<Option<Key>> {
    let mut handle = std::ptr::null_mut();
    let path = wide(RUN_KEY);
    let status = unsafe {
        if write {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                path.as_ptr(),
                0,
                std::ptr::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_QUERY_VALUE | KEY_SET_VALUE,
                std::ptr::null(),
                &mut handle,
                std::ptr::null_mut(),
            )
        } else {
            RegOpenKeyExW(
                HKEY_CURRENT_USER,
                path.as_ptr(),
                0,
                KEY_QUERY_VALUE,
                &mut handle,
            )
        }
    };
    if status == ERROR_FILE_NOT_FOUND {
        return Ok(None);
    }
    if status != ERROR_SUCCESS {
        bail!("打开自启注册表失败：{status}");
    }
    Ok(Some(Key(handle)))
}

pub(super) fn read_value() -> Result<Option<String>> {
    let Some(key) = open(false)? else {
        return Ok(None);
    };
    let name = wide(VALUE_NAME);
    let mut kind = 0;
    let mut size = 0;
    let status = unsafe {
        RegQueryValueExW(
            key.0,
            name.as_ptr(),
            std::ptr::null(),
            &mut kind,
            std::ptr::null_mut(),
            &mut size,
        )
    };
    if status == ERROR_FILE_NOT_FOUND {
        return Ok(None);
    }
    if status != ERROR_SUCCESS {
        bail!("读取自启设置失败：{status}");
    }
    if kind != REG_SZ || size == 0 || size > 16384 || size % 2 != 0 {
        bail!("自启项格式无效");
    }
    let mut buffer = vec![0u16; size as usize / 2];
    let status = unsafe {
        RegQueryValueExW(
            key.0,
            name.as_ptr(),
            std::ptr::null(),
            &mut kind,
            buffer.as_mut_ptr().cast(),
            &mut size,
        )
    };
    if status != ERROR_SUCCESS {
        bail!("读取自启目标失败：{status}");
    }
    let end = buffer.iter().position(|c| *c == 0).unwrap_or(buffer.len());
    if end + 1 != buffer.len() {
        bail!("自启项包含多余或缺失终止字符");
    }
    Ok(Some(
        String::from_utf16(&buffer[..end]).map_err(|e| anyhow!(e))?,
    ))
}

pub(super) fn write_value(value: Option<&str>) -> Result<()> {
    let key = open(true)?.ok_or_else(|| anyhow!("自启注册表不可用"))?;
    let name = wide(VALUE_NAME);
    let status = unsafe {
        if let Some(value) = value {
            let value = wide(value);
            RegSetValueExW(
                key.0,
                name.as_ptr(),
                0,
                REG_SZ,
                value.as_ptr().cast(),
                (value.len() * 2) as u32,
            )
        } else {
            RegDeleteValueW(key.0, name.as_ptr())
        }
    };
    if status != ERROR_SUCCESS && !(status == ERROR_FILE_NOT_FOUND && value.is_none()) {
        bail!("保存自启设置失败：{status}");
    }
    if read_value()?.as_deref() != value {
        bail!("系统未保留自启变更");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn registry_path_is_utf16_without_internal_nuls() {
        let path = wide(RUN_KEY);
        assert!(path[..path.len() - 1].iter().all(|c| *c != 0));
        assert_eq!(
            String::from_utf16(&path[..path.len() - 1]).unwrap(),
            RUN_KEY
        );
    }
}
