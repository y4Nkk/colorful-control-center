//! Windows 电源计划传输；测试实例不能退回真实系统 API。
use anyhow::{Context, Result, anyhow, bail, ensure};
use std::{collections::BTreeSet, os::windows::ffi::OsStrExt, path::Path, ptr};
use windows_sys::{
    Win32::{
        Foundation::{ERROR_FILE_NOT_FOUND, ERROR_MORE_DATA, ERROR_NO_MORE_ITEMS, LocalFree},
        System::{Power::*, Registry::KEY_WRITE},
    },
    core::GUID,
};

pub const SCHEMES: [u128; 3] = [
    0x381b4222_f694_41f0_9685_ff5bb260df2e,
    0x8c5e7fda_e8bf_4a96_9a85_a6e23a8c635c,
    0xa1841308_3541_4fab_bc81_f71556f20b4a,
];
const PCI: GUID = GUID::from_u128(0x501a4d13_42af_4429_9fd1_a8218c268e20);
const ASPM: GUID = GUID::from_u128(0xee12f906_d277_404b_b6da_e5fa1a576df5);
const USB: GUID = GUID::from_u128(0x2a737441_1930_4402_8d77_b2bebba308a3);
const SELECTIVE_SUSPEND: GUID = GUID::from_u128(0x48e6b7a6_50f5_4782_a5d4_53bb8f07e226);
const VIDEO: GUID = GUID::from_u128(0x7516b95f_f776_4464_8c53_06167f40cc99);
const BRIGHTNESS: GUID = GUID::from_u128(0xaded5e82_b909_4619_9949_f5d71dac0bcb);
const BUTTONS: GUID = GUID::from_u128(0x4f971e89_eebd_4455_a8de_9e59040e7347);
const LID: GUID = GUID::from_u128(0x5ca83367_6e45_459f_a27b_476b1d01c936);
const BATTERY: GUID = GUID::from_u128(0xe73a048d_bf27_4f12_9731_8b2076e8891f);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatteryAction {
    Low,
    Critical,
}
impl BatteryAction {
    fn guid(self) -> GUID {
        GUID::from_u128(match self {
            Self::Low => 0xd8742dcb_3e6a_4b3c_b3fe_374623cdcf06,
            Self::Critical => 0x637ea02f_bbcb_4015_8e2c_a1c7b9c0b546,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BatteryLevel {
    Low,
    Critical,
    Reserve,
}
impl BatteryLevel {
    fn guid(self) -> GUID {
        GUID::from_u128(match self {
            Self::Low => 0x8183ba9a_e910_48da_8769_14ae6dc1170a,
            Self::Critical => 0x9a66d8d7_4ff7_4ef9_b5a2_5a326ca2a469,
            Self::Reserve => 0xf3c5027d_cd16_4930_aa6b_90db844a8f00,
        })
    }
}

pub trait PowerAccess: Send + Sync {
    fn lid_action(&self, scheme: u128) -> Result<[u32; 2]>;
    fn active(&self) -> Result<u128>;
    fn battery_action(&self, scheme: u128, action: BatteryAction) -> Result<u32>;
    fn schemes(&self) -> Result<Vec<u128>>;
    fn scheme_name(&self, scheme: u128) -> Result<String>;
    fn check_active_scheme_write(&self) -> Result<()>;
    fn usb(&self, scheme: u128) -> Result<[u32; 2]>;
    fn set_usb(&self, scheme: u128, ac: bool, value: u32) -> Result<()>;
    fn aspm(&self, scheme: u128) -> Result<[u32; 2]>;
    fn set_aspm(&self, scheme: u128, ac: bool, value: u32) -> Result<()>;
    fn activate(&self, scheme: u128) -> Result<()>;
    fn brightness(&self, scheme: u128) -> Result<[u32; 2]>;
    fn set_brightness(&self, scheme: u128, ac: bool, value: u32) -> Result<()>;
    fn battery_level(&self, scheme: u128, level: BatteryLevel) -> Result<[u32; 2]>;
    fn set_battery_level(
        &self,
        scheme: u128,
        level: BatteryLevel,
        ac: bool,
        value: u32,
    ) -> Result<()>;
}
pub struct NativePower;

pub(crate) fn format_guid(id: u128) -> String {
    let hex = format!("{id:032x}");
    format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    )
}

pub(crate) fn parse_guid(value: &str) -> Result<u128> {
    ensure!(
        value.len() == 36
            && [8, 13, 18, 23]
                .iter()
                .all(|&index| value.as_bytes()[index] == b'-'),
        "电源 GUID 格式无效"
    );
    let id = u128::from_str_radix(&value.replace('-', ""), 16).context("电源 GUID 无效")?;
    ensure!(id != 0 && value == format_guid(id), "电源 GUID 不是规范值");
    Ok(id)
}

fn scheme_id(guid: GUID) -> u128 {
    (u128::from(guid.data1) << 96)
        | (u128::from(guid.data2) << 80)
        | (u128::from(guid.data3) << 64)
        | u128::from(u64::from_be_bytes(guid.data4))
}
fn checked(status: u32, operation: &str) -> Result<()> {
    if status != 0 {
        return Err(anyhow!(
            "{operation}：{} ({status})",
            std::io::Error::from_raw_os_error(status as i32)
        ));
    }
    Ok(())
}

impl NativePower {
    pub(crate) fn import_scheme(&self, path: &Path, scheme: u128) -> Result<()> {
        let path: Vec<_> = path.as_os_str().encode_wide().chain([0]).collect();
        let mut guid = GUID::from_u128(scheme);
        let wanted = &mut guid as *mut GUID;
        let mut destination = wanted;
        let status =
            unsafe { PowerImportPowerScheme(ptr::null_mut(), path.as_ptr(), &mut destination) };
        let imported =
            (status == 0 && !destination.is_null()).then(|| scheme_id(unsafe { *destination }));
        if !destination.is_null() && destination != wanted {
            unsafe { LocalFree(destination.cast()) };
        }
        checked(status, "导入校准电源计划")?;
        ensure!(
            imported == Some(scheme),
            "导入返回的电源计划标识不符合本次任务"
        );
        Ok(())
    }

    pub(crate) fn delete_inactive_scheme(&self, scheme: u128) -> Result<()> {
        ensure!(self.active()? != scheme, "校准电源计划仍在使用，未删除");
        let status = unsafe { PowerDeleteScheme(ptr::null_mut(), &GUID::from_u128(scheme)) };
        if status == ERROR_FILE_NOT_FOUND {
            return Ok(());
        }
        checked(status, "删除校准电源计划")
    }
}

impl PowerAccess for NativePower {
    fn lid_action(&self, scheme: u128) -> Result<[u32; 2]> {
        let values = read_setting(scheme, &BUTTONS, &LID, "读取合盖计划动作")?;
        if values.iter().any(|value| *value > 3) {
            bail!("Windows合盖动作编号未知");
        }
        Ok(values)
    }
    fn battery_action(&self, scheme: u128, action: BatteryAction) -> Result<u32> {
        read_value(scheme, &BATTERY, &action.guid(), false, "读取电池计划动作")
    }
    fn battery_level(&self, scheme: u128, level: BatteryLevel) -> Result<[u32; 2]> {
        let values = read_setting(scheme, &BATTERY, &level.guid(), "读取电池计划阈值")?;
        if values.iter().any(|value| *value > 100) {
            bail!("电池计划阈值超出百分比范围");
        }
        Ok(values)
    }
    fn set_battery_level(
        &self,
        scheme: u128,
        level: BatteryLevel,
        ac: bool,
        value: u32,
    ) -> Result<()> {
        if value > 100 {
            bail!("电池计划阈值超出百分比范围");
        }
        write_setting(
            scheme,
            &BATTERY,
            &level.guid(),
            ac,
            value,
            "写入电池计划阈值",
        )
    }
    fn brightness(&self, scheme: u128) -> Result<[u32; 2]> {
        let values = read_setting(scheme, &VIDEO, &BRIGHTNESS, "读取屏幕计划亮度")?;
        if values.iter().any(|value| *value > 100) {
            bail!("屏幕计划亮度超出百分比范围");
        }
        Ok(values)
    }
    fn set_brightness(&self, scheme: u128, ac: bool, value: u32) -> Result<()> {
        if value > 100 {
            bail!("屏幕计划亮度超出百分比范围");
        }
        write_setting(scheme, &VIDEO, &BRIGHTNESS, ac, value, "写入屏幕计划亮度")
    }
    fn schemes(&self) -> Result<Vec<u128>> {
        let mut schemes = Vec::new();
        let mut seen = BTreeSet::new();
        for index in 0..=u32::MAX {
            let mut guid = GUID::default();
            let mut size = std::mem::size_of::<GUID>() as u32;
            let status = unsafe {
                PowerEnumerate(
                    std::ptr::null_mut(),
                    std::ptr::null(),
                    std::ptr::null(),
                    ACCESS_SCHEME,
                    index,
                    (&mut guid as *mut GUID).cast(),
                    &mut size,
                )
            };
            if status == ERROR_NO_MORE_ITEMS {
                return Ok(schemes);
            }
            checked(status, "枚举电源计划")?;
            if size != std::mem::size_of::<GUID>() as u32 {
                bail!("电源计划GUID长度无效");
            }
            let id = scheme_id(guid);
            ensure!(
                id != 0 && seen.insert(id),
                "Windows 返回无效或重复的电源计划 GUID"
            );
            schemes.push(id);
        }
        bail!("Windows 电源计划枚举索引耗尽")
    }
    fn scheme_name(&self, scheme: u128) -> Result<String> {
        let guid = GUID::from_u128(scheme);
        read_friendly_name(|buffer, size| unsafe {
            PowerReadFriendlyName(
                std::ptr::null_mut(),
                &guid,
                std::ptr::null(),
                std::ptr::null(),
                buffer,
                size,
            )
        })
    }
    fn check_active_scheme_write(&self) -> Result<()> {
        checked(
            unsafe { PowerSettingAccessCheckEx(ACCESS_ACTIVE_SCHEME, ptr::null(), KEY_WRITE) },
            "Windows 电源计划切换权限检查",
        )
    }
    fn usb(&self, scheme: u128) -> Result<[u32; 2]> {
        read_setting(scheme, &USB, &SELECTIVE_SUSPEND, "读取USB 选择性暂停")
    }
    fn set_usb(&self, scheme: u128, ac: bool, value: u32) -> Result<()> {
        write_setting(
            scheme,
            &USB,
            &SELECTIVE_SUSPEND,
            ac,
            value,
            "写入USB 选择性暂停",
        )
    }
    fn active(&self) -> Result<u128> {
        let mut ptr = std::ptr::null_mut();
        unsafe {
            checked(
                PowerGetActiveScheme(std::ptr::null_mut(), &mut ptr),
                "读取活动电源计划",
            )?;
            if ptr.is_null() {
                return Err(anyhow!("活动电源计划返回空指针"));
            }
            let guid = *ptr;
            LocalFree(ptr.cast());
            Ok(scheme_id(guid))
        }
    }
    fn aspm(&self, scheme: u128) -> Result<[u32; 2]> {
        read_setting(scheme, &PCI, &ASPM, "读取链路节能设置")
    }
    fn set_aspm(&self, scheme: u128, ac: bool, value: u32) -> Result<()> {
        write_setting(scheme, &PCI, &ASPM, ac, value, "写入链路节能设置")
    }
    fn activate(&self, scheme: u128) -> Result<()> {
        unsafe {
            checked(
                PowerSetActiveScheme(std::ptr::null_mut(), &GUID::from_u128(scheme)),
                "激活电源计划",
            )?;
        }
        if self.active()? != scheme {
            return Err(anyhow!("活动电源计划回读不一致"));
        }
        Ok(())
    }
}

fn read_friendly_name(mut read: impl FnMut(*mut u8, &mut u32) -> u32) -> Result<String> {
    let mut size = 0;
    checked(read(ptr::null_mut(), &mut size), "读取电源计划名称长度")?;
    for _ in 0..3 {
        ensure!(size.is_multiple_of(2), "电源计划名称长度无效");
        if size == 0 {
            return Ok(String::new());
        }
        let capacity = size;
        let mut text = vec![0u16; capacity as usize / 2];
        let status = read(text.as_mut_ptr().cast(), &mut size);
        if status == ERROR_MORE_DATA {
            ensure!(size > capacity, "Windows 未返回增加后的电源计划名称长度");
            continue;
        }
        checked(status, "读取电源计划名称")?;
        ensure!(
            size.is_multiple_of(2) && size <= capacity,
            "电源计划名称长度无效"
        );
        let text = &text[..size as usize / 2];
        let end = text
            .iter()
            .position(|&unit| unit == 0)
            .unwrap_or(text.len());
        return String::from_utf16(&text[..end]).context("电源计划名称不是有效的 UTF-16");
    }
    bail!("电源计划名称在读取时持续变化，请重新读取")
}

fn read_setting(scheme: u128, group: &GUID, setting: &GUID, operation: &str) -> Result<[u32; 2]> {
    Ok([
        read_value(scheme, group, setting, true, operation)?,
        read_value(scheme, group, setting, false, operation)?,
    ])
}
pub(crate) fn read_value(
    scheme: u128,
    group: &GUID,
    setting: &GUID,
    ac: bool,
    operation: &str,
) -> Result<u32> {
    let scheme = GUID::from_u128(scheme);
    let mut value = 0;
    let read = if ac {
        PowerReadACValueIndex
    } else {
        PowerReadDCValueIndex
    };
    let status = unsafe { read(std::ptr::null_mut(), &scheme, group, setting, &mut value) };
    checked(
        status,
        &format!("{operation}（{}）", if ac { "插电" } else { "电池" }),
    )?;
    Ok(value)
}
fn write_setting(
    scheme: u128,
    group: &GUID,
    setting: &GUID,
    ac: bool,
    value: u32,
    operation: &str,
) -> Result<()> {
    let guid = GUID::from_u128(scheme);
    let status = unsafe {
        if ac {
            PowerWriteACValueIndex(std::ptr::null_mut(), &guid, group, setting, value)
        } else {
            PowerWriteDCValueIndex(std::ptr::null_mut(), &guid, group, setting, value)
        }
    };
    checked(
        status,
        &format!("{operation}（{}）", if ac { "插电" } else { "电池" }),
    )?;
    if read_value(scheme, group, setting, ac, operation)? != value {
        bail!("{operation}回读不一致");
    }
    Ok(())
}

#[cfg(test)]
pub struct UnavailablePower;
#[cfg(test)]
impl PowerAccess for UnavailablePower {
    fn lid_action(&self, _: u128) -> Result<[u32; 2]> {
        Err(anyhow!("隔离测试未注入合盖计划动作"))
    }
    fn battery_action(&self, _: u128, _: BatteryAction) -> Result<u32> {
        Err(anyhow!("隔离测试未注入电池计划动作"))
    }
    fn battery_level(&self, _: u128, _: BatteryLevel) -> Result<[u32; 2]> {
        Err(anyhow!("隔离测试未注入电池计划阈值"))
    }
    fn set_battery_level(&self, _: u128, _: BatteryLevel, _: bool, _: u32) -> Result<()> {
        Err(anyhow!("隔离测试未注入电池计划阈值"))
    }
    fn brightness(&self, _: u128) -> Result<[u32; 2]> {
        self.active().map(|_| [0; 2])
    }
    fn set_brightness(&self, _: u128, _: bool, _: u32) -> Result<()> {
        self.active().map(|_| ())
    }
    fn schemes(&self) -> Result<Vec<u128>> {
        self.active().map(|_| vec![])
    }
    fn scheme_name(&self, _: u128) -> Result<String> {
        bail!("隔离测试未注入电源计划名称")
    }
    fn check_active_scheme_write(&self) -> Result<()> {
        bail!("隔离测试未注入电源计划写权限")
    }
    fn usb(&self, _: u128) -> Result<[u32; 2]> {
        self.active().map(|_| [0; 2])
    }
    fn set_usb(&self, _: u128, _: bool, _: u32) -> Result<()> {
        self.active().map(|_| ())
    }
    fn active(&self) -> Result<u128> {
        Err(anyhow!("隔离测试未注入电源计划通道"))
    }
    fn aspm(&self, _: u128) -> Result<[u32; 2]> {
        self.active().map(|_| [0; 2])
    }
    fn set_aspm(&self, _: u128, _: bool, _: u32) -> Result<()> {
        self.active().map(|_| ())
    }
    fn activate(&self, _: u128) -> Result<()> {
        self.active().map(|_| ())
    }
}

#[cfg(test)]
mod tests;
