//! 与FnKey相同的只读I2C识别和Windows触摸板切换；不写系统状态注册表。
use anyhow::{Context, Result, bail};
use std::{
    ptr, thread,
    time::{Duration, Instant},
};
use windows_sys::{
    Win32::{
        Devices::DeviceAndDriverInstallation::*, Foundation::*, System::Registry::*,
        UI::Input::KeyboardAndMouse::*,
    },
    core::GUID,
};

const HID_CLASS: GUID = GUID::from_u128(0x745a17a0_74d3_11d0_b6fe_00a0c90f57da);
const STATUS: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\PrecisionTouchPad\Status";
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}
struct Devices(HDEVINFO);
impl Drop for Devices {
    fn drop(&mut self) {
        unsafe {
            SetupDiDestroyDeviceInfoList(self.0);
        }
    }
}
fn text(buffer: &[u16]) -> Result<String> {
    let end = buffer
        .iter()
        .position(|v| *v == 0)
        .context("触摸板设备文本缺少结束符")?;
    String::from_utf16(&buffer[..end]).context("触摸板设备文本无效")
}
fn identity(set: &Devices, info: &SP_DEVINFO_DATA) -> Result<Option<String>> {
    let mut description = [0u16; 4096];
    if unsafe {
        SetupDiGetDeviceRegistryPropertyW(
            set.0,
            info,
            SPDRP_DEVICEDESC,
            ptr::null_mut(),
            description.as_mut_ptr().cast(),
            (description.len() * 2) as u32,
            ptr::null_mut(),
        )
    } == 0
    {
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() == Some(ERROR_INVALID_DATA as i32) {
            return Ok(None);
        }
        return Err(error.into());
    }
    if !text(&description)?.contains("I2C") {
        return Ok(None);
    }
    let (mut status, mut problem) = (0, 0);
    let code = unsafe { CM_Get_DevNode_Status(&mut status, &mut problem, info.DevInst, 0) };
    if code == CR_NO_SUCH_DEVNODE || code == CR_INVALID_DEVNODE {
        return Ok(None);
    }
    if code != CR_SUCCESS {
        bail!("读取I2C设备状态失败：{code}");
    }
    let mut buffer = [0u16; 4096];
    if unsafe {
        SetupDiGetDeviceInstanceIdW(
            set.0,
            info,
            buffer.as_mut_ptr(),
            buffer.len() as u32,
            ptr::null_mut(),
        )
    } == 0
    {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(Some(text(&buffer)?.to_ascii_uppercase()))
}
fn devices() -> Result<Vec<String>> {
    let raw =
        unsafe { SetupDiGetClassDevsW(&HID_CLASS, ptr::null(), ptr::null_mut(), DIGCF_PRESENT) };
    if raw == -1isize {
        return Err(std::io::Error::last_os_error().into());
    }
    let set = Devices(raw);
    let mut ids = Vec::new();
    for index in 0..4096 {
        let mut info: SP_DEVINFO_DATA = unsafe { std::mem::zeroed() };
        info.cbSize = std::mem::size_of_val(&info) as u32;
        if unsafe { SetupDiEnumDeviceInfo(set.0, index, &mut info) } == 0 {
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() == Some(ERROR_NO_MORE_ITEMS as i32) {
                ids.sort();
                ids.dedup();
                return Ok(ids);
            }
            return Err(error.into());
        }
        if let Some(id) = identity(&set, &info)? {
            ids.push(id);
        }
        if ids.len() > 128 {
            bail!("I2C识别设备过多");
        }
    }
    bail!("HID设备枚举超过支持数量")
}
fn enabled() -> Result<Option<bool>> {
    let mut value = 0u32;
    let mut size = std::mem::size_of_val(&value) as u32;
    let code = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            wide(STATUS).as_ptr(),
            wide("Enabled").as_ptr(),
            RRF_RT_REG_DWORD,
            ptr::null_mut(),
            (&mut value as *mut u32).cast(),
            &mut size,
        )
    };
    if matches!(
        code,
        ERROR_FILE_NOT_FOUND | ERROR_PATH_NOT_FOUND | ERROR_UNSUPPORTED_TYPE
    ) {
        return Ok(None);
    }
    if code != ERROR_SUCCESS {
        bail!("读取Windows触摸板状态失败：{code}");
    }
    Ok(match value {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    })
}
pub(crate) fn capture() -> Result<(Vec<String>, Option<bool>)> {
    Ok((devices()?, enabled()?))
}
fn input(key: u16, up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: key,
                wScan: 0,
                dwFlags: if up { KEYEVENTF_KEYUP } else { 0 },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}
fn toggle() -> Result<()> {
    for key in [VK_SHIFT, VK_CONTROL, VK_MENU, VK_LWIN, VK_RWIN, VK_F24] {
        if unsafe { GetAsyncKeyState(key as i32) } < 0 {
            bail!("请松开修饰键后重试触摸板切换");
        }
    }
    let events = [
        input(VK_CONTROL, false),
        input(VK_LWIN, false),
        input(VK_F24, false),
        input(VK_CONTROL, true),
        input(VK_LWIN, true),
        input(VK_F24, true),
    ];
    let sent = unsafe {
        SendInput(
            events.len() as u32,
            events.as_ptr(),
            std::mem::size_of::<INPUT>() as i32,
        )
    };
    if sent == events.len() as u32 {
        return Ok(());
    }
    let mut held = std::collections::BTreeSet::new();
    for event in events.iter().take(sent as usize) {
        let key = unsafe { event.Anonymous.ki };
        if key.dwFlags & KEYEVENTF_KEYUP == 0 {
            held.insert(key.wVk);
        } else {
            held.remove(&key.wVk);
        }
    }
    let cleanup: Vec<_> = held.into_iter().map(|key| input(key, true)).collect();
    if !cleanup.is_empty() {
        let released = unsafe {
            SendInput(
                cleanup.len() as u32,
                cleanup.as_ptr(),
                std::mem::size_of::<INPUT>() as i32,
            )
        };
        if released != cleanup.len() as u32 {
            bail!("触摸板切换输入不完整，修饰键释放也失败");
        }
    }
    bail!("触摸板切换输入不完整：{sent}/6，已释放本次按下的修饰键")
}
pub(crate) fn set_enabled(expected: &[String], before: bool, desired: bool) -> Result<()> {
    let (ids, state) = capture()?;
    if ids != expected || ids.is_empty() || state != Some(before) {
        bail!("触摸板设备或系统状态已变化，未发送切换");
    }
    if before == desired {
        return Ok(());
    }
    toggle()?;
    let until = Instant::now() + Duration::from_secs(2);
    loop {
        let (ids, state) = capture()?;
        if ids != expected {
            bail!("触摸板切换期间设备集合变化");
        }
        if state == Some(desired) {
            return Ok(());
        }
        if Instant::now() >= until {
            bail!("Windows未确认触摸板目标状态，请检查会话权限后重试");
        }
        thread::sleep(Duration::from_millis(50));
    }
}
