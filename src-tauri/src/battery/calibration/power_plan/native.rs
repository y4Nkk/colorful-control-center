use super::{Access, RESOURCE, bundle, format, parse};
use crate::hal::power_plan::{NativePower, PowerAccess};
use anyhow::{Context, Result, bail};
use std::{io::Read, os::windows::ffi::OsStrExt, path::PathBuf};
use windows_sys::{
    Win32::{
        Foundation::{ERROR_FILE_NOT_FOUND, ERROR_NO_MORE_ITEMS, LocalFree},
        System::Power::*,
    },
    core::GUID,
};

pub(in crate::battery::calibration) struct Native {
    pub resource_path: PathBuf,
}
fn checked(status: u32, action: &str) -> Result<()> {
    if status != 0 {
        bail!(
            "{action}：{} ({status})",
            std::io::Error::from_raw_os_error(status as i32)
        );
    }
    Ok(())
}
fn number(guid: GUID) -> u128 {
    (u128::from(guid.data1) << 96)
        | (u128::from(guid.data2) << 80)
        | (u128::from(guid.data3) << 64)
        | u128::from(u64::from_be_bytes(guid.data4))
}
impl Native {
    fn resource_exists(&self) -> Result<bool> {
        let file = match std::fs::File::open(&self.resource_path) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(e) => return Err(e.into()),
        };
        let mut bytes = Vec::new();
        file.take(RESOURCE.len() as u64 + 1)
            .read_to_end(&mut bytes)?;
        if bytes != RESOURCE {
            bail!(
                "校准资源 {} 包含其他内容，已保留文件",
                self.resource_path.display()
            );
        }
        Ok(true)
    }
    fn prepare_resource(&self) -> Result<()> {
        bundle()?;
        if !self.resource_exists()? {
            std::fs::create_dir_all(self.resource_path.parent().context("校准资源目录无效")?)?;
            crate::storage::write(&self.resource_path, RESOURCE)?;
        }
        Ok(())
    }
}
impl Access for Native {
    fn active(&self) -> Result<String> {
        Ok(format(NativePower.active()?))
    }
    fn exists(&self, id: &str) -> Result<bool> {
        let expected = parse(id)?;
        for index in 0..4096 {
            let mut guid = GUID::default();
            let mut size = std::mem::size_of::<GUID>() as u32;
            let code = unsafe {
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
            if code == ERROR_NO_MORE_ITEMS {
                return Ok(false);
            }
            checked(code, "枚举电源计划")?;
            if size != std::mem::size_of::<GUID>() as u32 {
                bail!("电源计划标识长度无效");
            }
            if number(guid) == expected {
                return Ok(true);
            }
        }
        bail!("电源计划数量超过校准检查范围");
    }
    fn import(&self, id: &str) -> Result<()> {
        let expected = parse(id)?;
        self.prepare_resource()?;
        let path: Vec<_> = self
            .resource_path
            .as_os_str()
            .encode_wide()
            .chain([0])
            .collect();
        let mut desired = GUID::from_u128(expected);
        let original = &mut desired as *mut GUID;
        let mut destination = original;
        let code = unsafe {
            PowerImportPowerScheme(std::ptr::null_mut(), path.as_ptr(), &mut destination)
        };
        let imported = if destination.is_null() {
            None
        } else {
            Some(number(unsafe { *destination }))
        };
        if !destination.is_null() && destination != original {
            unsafe {
                LocalFree(destination.cast());
            }
        }
        checked(code, "导入本次校准电源计划")?;
        if imported != Some(expected) {
            bail!("导入返回的电源计划标识不符合本次任务");
        }
        Ok(())
    }
    fn verify(&self, id: &str) -> Result<()> {
        let scheme = GUID::from_u128(parse(id)?);
        for entry in bundle()?.entries {
            let group = GUID::from_u128(parse(&entry.subgroup)?);
            let setting = GUID::from_u128(parse(&entry.setting)?);
            for (ac, wanted) in [(true, entry.ac), (false, entry.dc)] {
                let Some(wanted) = wanted else {
                    continue;
                };
                let mut actual = 0;
                let code = unsafe {
                    if ac {
                        PowerReadACValueIndex(
                            std::ptr::null_mut(),
                            &scheme,
                            &group,
                            &setting,
                            &mut actual,
                        )
                    } else {
                        PowerReadDCValueIndex(
                            std::ptr::null_mut(),
                            &scheme,
                            &group,
                            &setting,
                            &mut actual,
                        )
                    }
                };
                checked(code, "回读校准电源策略")?;
                if actual != wanted {
                    bail!(
                        "校准电源策略 {} 的{}值不一致",
                        entry.setting,
                        if ac { "AC" } else { "DC" }
                    );
                }
            }
        }
        Ok(())
    }
    fn activate(&self, id: &str) -> Result<()> {
        NativePower.activate(parse(id)?)
    }
    fn delete(&self, id: &str) -> Result<()> {
        let number = parse(id)?;
        if NativePower.active()? == number {
            bail!("校准电源计划仍在使用，未删除");
        }
        let code = unsafe { PowerDeleteScheme(std::ptr::null_mut(), &GUID::from_u128(number)) };
        if code == ERROR_FILE_NOT_FOUND {
            return Ok(());
        }
        checked(code, "删除本次校准电源计划")
    }
    fn cleanup(&self) -> Result<()> {
        if self.resource_exists()? {
            std::fs::remove_file(&self.resource_path)?;
        }
        Ok(())
    }
}
