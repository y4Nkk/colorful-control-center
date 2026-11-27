use super::*;
use windows_sys::Win32::Graphics::Gdi::*;

#[path = "display/embedded.rs"]
mod embedded;
pub(crate) use embedded::{EmbeddedPanel, embedded_panel};

pub struct Native;
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}
fn text(chars: &[u16]) -> String {
    String::from_utf16_lossy(&chars[..chars.iter().position(|v| *v == 0).unwrap_or(chars.len())])
}
fn get(device: &str, index: u32) -> Result<DEVMODEW> {
    let mut mode: DEVMODEW = unsafe { std::mem::zeroed() };
    mode.dmSize = std::mem::size_of::<DEVMODEW>() as u16;
    if unsafe { EnumDisplaySettingsW(wide(device).as_ptr(), index, &mut mode) } == 0 {
        bail!("读取显示模式失败：{}", std::io::Error::last_os_error());
    }
    Ok(mode)
}
fn portable(device: &DISPLAY_DEVICEW, raw: &DEVMODEW) -> Mode {
    unsafe {
        Mode {
            device: text(&device.DeviceName),
            identity: text(&device.DeviceID),
            name: text(&device.DeviceString),
            width: raw.dmPelsWidth,
            height: raw.dmPelsHeight,
            bits: raw.dmBitsPerPel,
            orientation: raw.Anonymous1.Anonymous2.dmDisplayOrientation,
            x: raw.Anonymous1.Anonymous2.dmPosition.x,
            y: raw.Anonymous1.Anonymous2.dmPosition.y,
            flags: raw.Anonymous2.dmDisplayFlags,
            frequency: raw.dmDisplayFrequency,
        }
    }
}
fn geometry(a: &Mode, b: &Mode) -> bool {
    a.width == b.width
        && a.height == b.height
        && a.bits == b.bits
        && a.orientation == b.orientation
        && a.flags == b.flags
}
impl Access for Native {
    fn list(&self) -> Result<Vec<Display>> {
        let mut displays = Vec::new();
        for index in 0..256 {
            let mut device: DISPLAY_DEVICEW = unsafe { std::mem::zeroed() };
            device.cb = std::mem::size_of_val(&device) as u32;
            if unsafe { EnumDisplayDevicesW(std::ptr::null(), index, &mut device, 0) } == 0 {
                break;
            }
            if device.StateFlags & DISPLAY_DEVICE_ATTACHED_TO_DESKTOP == 0
                || device.StateFlags & DISPLAY_DEVICE_MIRRORING_DRIVER != 0
            {
                continue;
            }
            let current = portable(
                &device,
                &get(&text(&device.DeviceName), ENUM_CURRENT_SETTINGS)?,
            );
            let mut frequencies = Vec::new();
            for mode_index in 0..4096 {
                let Ok(raw) = get(&current.device, mode_index) else {
                    break;
                };
                let candidate = portable(&device, &raw);
                if geometry(&current, &candidate) && candidate.frequency > 1 {
                    frequencies.push(candidate.frequency);
                }
            }
            frequencies.sort_unstable();
            frequencies.dedup();
            displays.push(Display {
                current,
                frequencies,
                primary: device.StateFlags & DISPLAY_DEVICE_PRIMARY_DEVICE != 0,
            });
        }
        Ok(displays)
    }
    fn test(&self, mode: &Mode) -> Result<()> {
        change(mode, CDS_TEST)
    }
    fn set(&self, mode: &Mode) -> Result<()> {
        change(mode, 0)?;
        let actual = self
            .list()?
            .into_iter()
            .find(|d| d.current.device == mode.device)
            .ok_or_else(|| anyhow!("显示器已断开"))?;
        if !actual.current.same_state(mode) {
            bail!("显示模式写后回读不一致");
        }
        Ok(())
    }
}
fn change(mode: &Mode, flags: u32) -> Result<()> {
    let actual = Native
        .list()?
        .into_iter()
        .find(|d| d.current.device == mode.device)
        .ok_or_else(|| anyhow!("显示器已断开"))?;
    if actual.current.identity != mode.identity {
        bail!("显示器身份已变化，拒绝替换设备");
    }
    let mut raw = get(&mode.device, ENUM_CURRENT_SETTINGS)?;
    raw.dmPelsWidth = mode.width;
    raw.dmPelsHeight = mode.height;
    raw.dmBitsPerPel = mode.bits;
    raw.dmDisplayFrequency = mode.frequency;
    raw.Anonymous1.Anonymous2.dmPosition.x = mode.x;
    raw.Anonymous1.Anonymous2.dmPosition.y = mode.y;
    raw.Anonymous1.Anonymous2.dmDisplayOrientation = mode.orientation;
    raw.Anonymous2.dmDisplayFlags = mode.flags;
    raw.dmFields = DM_PELSWIDTH
        | DM_PELSHEIGHT
        | DM_BITSPERPEL
        | DM_DISPLAYFREQUENCY
        | DM_POSITION
        | DM_DISPLAYORIENTATION
        | DM_DISPLAYFLAGS;
    let status = unsafe {
        ChangeDisplaySettingsExW(
            wide(&mode.device).as_ptr(),
            &raw,
            std::ptr::null_mut(),
            flags,
            std::ptr::null(),
        )
    };
    if status != DISP_CHANGE_SUCCESSFUL {
        bail!("Windows 显示切换失败 ({status})，未请求重启或写注册表");
    }
    Ok(())
}
