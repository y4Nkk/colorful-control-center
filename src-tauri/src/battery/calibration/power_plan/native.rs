use super::{Access, RESOURCE, bundle};
use crate::hal::power_plan::{NativePower, PowerAccess, format_guid, parse_guid, read_value};
use anyhow::{Context, Result, bail};
use std::{io::Read, path::PathBuf};
use windows_sys::core::GUID;

pub(in crate::battery::calibration) struct Native {
    pub resource_path: PathBuf,
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
        Ok(format_guid(NativePower.active()?))
    }
    fn exists(&self, id: &str) -> Result<bool> {
        let id = parse_guid(id)?;
        Ok(NativePower.schemes()?.contains(&id))
    }
    fn import(&self, id: &str) -> Result<()> {
        let id = parse_guid(id)?;
        self.prepare_resource()?;
        NativePower.import_scheme(&self.resource_path, id)
    }
    fn verify(&self, id: &str) -> Result<()> {
        let scheme = parse_guid(id)?;
        for entry in bundle()?.entries {
            let group = GUID::from_u128(parse_guid(&entry.subgroup)?);
            let setting = GUID::from_u128(parse_guid(&entry.setting)?);
            for (ac, wanted) in [(true, entry.ac), (false, entry.dc)] {
                let Some(wanted) = wanted else {
                    continue;
                };
                let actual = read_value(scheme, &group, &setting, ac, "回读校准电源策略")?;
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
        NativePower.activate(parse_guid(id)?)
    }
    fn delete(&self, id: &str) -> Result<()> {
        NativePower.delete_inactive_scheme(parse_guid(id)?)
    }
    fn cleanup(&self) -> Result<()> {
        if self.resource_exists()? {
            std::fs::remove_file(&self.resource_path)?;
        }
        Ok(())
    }
}
