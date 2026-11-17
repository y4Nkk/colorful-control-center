use super::*;
use crate::hal::oem_dchu::Dchu;
use windows_sys::{
    Win32::{Devices::DeviceAndDriverInstallation::*, Graphics::Gdi::*},
    core::GUID,
};
#[path = "gpu_device/protocol.rs"]
pub(crate) mod protocol;
const CLASS: GUID = GUID::from_u128(0x62f9c741_b25a_46ce_b54c_9bccce08b6f2);
fn text(chars: &[u16]) -> String {
    String::from_utf16_lossy(&chars[..chars.iter().position(|v| *v == 0).unwrap_or(chars.len())])
}
struct Devices(HDEVINFO);
impl Drop for Devices {
    fn drop(&mut self) {
        unsafe {
            SetupDiDestroyDeviceInfoList(self.0);
        }
    }
}
fn properties(set: &Devices, device: &SP_DEVINFO_DATA, key: u32) -> Result<Vec<String>> {
    let mut buf = [0u16; 4096];
    if unsafe {
        SetupDiGetDeviceRegistryPropertyW(
            set.0,
            device,
            key,
            std::ptr::null_mut(),
            buf.as_mut_ptr().cast(),
            (buf.len() * 2) as u32,
            std::ptr::null_mut(),
        )
    } == 0
    {
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() == Some(13) {
            return Ok(Vec::new());
        }
        return Err(error.into());
    }
    Ok(buf
        .split(|v| *v == 0)
        .take_while(|v| !v.is_empty())
        .map(String::from_utf16_lossy)
        .collect())
}
fn active_outputs() -> Result<(bool, bool)> {
    let (mut nvidia, mut alternative) = (false, false);
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
        let id = text(&device.DeviceID).to_ascii_uppercase();
        if id.is_empty() {
            bail!("活动显示器的适配器身份未知，不能确认原厂关断影响");
        }
        nvidia |= id.starts_with("PCI\\VEN_10DE&");
        alternative |= id.starts_with("PCI\\VEN_") && !id.starts_with("PCI\\VEN_10DE&");
    }
    Ok((nvidia, alternative))
}
fn item(
    set: &Devices,
    info: &SP_DEVINFO_DATA,
    outputs: (bool, bool),
    automatic: bool,
) -> Result<Option<(Device, u32)>> {
    if !properties(set, info, SPDRP_HARDWAREID)?
        .iter()
        .any(|id| id.eq_ignore_ascii_case("ACPI\\NVDA0820"))
    {
        return Ok(None);
    }
    let mut buf = [0u16; 512];
    if unsafe {
        SetupDiGetDeviceInstanceIdW(
            set.0,
            info,
            buf.as_mut_ptr(),
            buf.len() as u32,
            std::ptr::null_mut(),
        )
    } == 0
    {
        bail!("读取原厂独显控制实例失败");
    }
    let id = text(&buf).to_ascii_uppercase();
    let (mut status, mut problem) = (0, 0);
    let code = unsafe { CM_Get_DevNode_Status(&mut status, &mut problem, info.DevInst, 0) };
    if code == CR_NO_SUCH_DEVNODE || code == CR_INVALID_DEVNODE {
        return Ok(None);
    }
    if code != CR_SUCCESS {
        return Err(operation::NativeCode(code).into());
    }
    let name = properties(set, info, SPDRP_DEVICEDESC)?
        .into_iter()
        .next()
        .unwrap_or_else(|| "原厂独显电源控制器".into());
    Ok(Some((
        Device {
            id,
            name,
            enabled: problem != 22,
            automatic_power_off: automatic,
            problem,
            drives_display: outputs.0,
            can_disable: !outputs.0 && outputs.1,
        },
        info.DevInst,
    )))
}
fn nodes(automatic: bool) -> Result<Vec<(Device, u32)>> {
    let handle = unsafe { SetupDiGetClassDevsW(&CLASS, std::ptr::null(), std::ptr::null_mut(), 0) };
    if handle == -1isize {
        return Err(std::io::Error::last_os_error().into());
    }
    let set = Devices(handle);
    let outputs = active_outputs()?;
    let mut found = Vec::new();
    for index in 0..256 {
        let mut info: SP_DEVINFO_DATA = unsafe { std::mem::zeroed() };
        info.cbSize = std::mem::size_of_val(&info) as u32;
        if unsafe { SetupDiEnumDeviceInfo(set.0, index, &mut info) } == 0 {
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() != Some(259) {
                return Err(error.into());
            }
            break;
        }
        if let Some(node) = item(&set, &info, outputs, automatic)? {
            found.push(node);
        }
    }
    if found.len() > 1 {
        bail!("存在多个原厂独显控制节点，无法绑定全局固件开关");
    }
    Ok(found)
}
pub(crate) fn supported(dchu: &Dchu) -> Result<bool> {
    let caps = dchu.read_bios_capabilities(0, 20)?;
    protocol::supported(&caps)
}
pub(crate) fn list(dchu: &Dchu) -> Result<Vec<Device>> {
    if !supported(dchu)? {
        return Ok(Vec::new());
    }
    let flag = dchu.read_settings(1, 5, 1)?[0];
    nodes(protocol::automatic(flag)?)
        .map(|nodes| nodes.into_iter().map(|(device, _)| device).collect())
}
pub(crate) fn inspect(dchu: &Dchu, id: &str) -> Result<Device> {
    list(dchu)?
        .into_iter()
        .find(|device| device.id == id)
        .ok_or_else(|| anyhow!("原厂独显控制实例不存在或能力已变化"))
}
pub(crate) fn set_node(id: &str, enabled: bool) -> Result<()> {
    let (device, instance) = nodes(false)?
        .into_iter()
        .find(|(device, _)| device.id == id)
        .ok_or_else(|| anyhow!("原厂独显控制实例已移除"))?;
    if !device.healthy() {
        bail!("原厂控制节点存在其他故障");
    }
    if !enabled && !device.can_disable {
        bail!("显示输出已变化，拒绝关断独显");
    }
    let code = unsafe {
        if enabled {
            CM_Enable_DevNode(instance, 0)
        } else {
            CM_Disable_DevNode(instance, 0)
        }
    };
    if code != CR_SUCCESS {
        return Err(operation::NativeCode(code).into());
    }
    Ok(())
}
