use crate::hal::power_plan::parse_guid;
use anyhow::{Result, bail};
use serde::Deserialize;
use sha2::{Digest, Sha256};
#[path = "power_plan/native.rs"]
pub(super) mod native;
#[cfg(test)]
#[path = "power_plan/tests.rs"]
mod tests;

pub(crate) const RESOURCE: &[u8] = include_bytes!("../../../resources/oem/battery-utility.pow");
const INDEX: &str = include_str!("../../../resources/oem/battery-utility.json");
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Bundle {
    pub sha256: String,
    pub entries: Vec<Entry>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Entry {
    pub subgroup: String,
    pub setting: String,
    pub ac: Option<u32>,
    pub dc: Option<u32>,
}
pub(crate) fn bundle() -> Result<Bundle> {
    let bundle: Bundle = serde_json::from_str(INDEX)?;
    if format!("{:x}", Sha256::digest(RESOURCE)) != bundle.sha256 || bundle.entries.len() != 15 {
        bail!("校准电源资源与索引清单不一致");
    }
    for entry in &bundle.entries {
        parse_guid(&entry.subgroup)?;
        parse_guid(&entry.setting)?;
        if entry.ac.is_none() && entry.dc.is_none() {
            bail!("校准电源索引为空");
        }
    }
    Ok(bundle)
}
pub(crate) fn owned(operation: &str) -> Result<u128> {
    if operation.len() != 32 || !operation.bytes().all(|c| c.is_ascii_hexdigit()) {
        bail!("校准操作标识无效");
    }
    let hash = Sha256::digest(format!("ccc.calibration.power.v1:{operation}"));
    let value = u128::from_be_bytes(hash[..16].try_into()?);
    Ok((value & !(15 << 76) & !(3 << 62)) | (8 << 76) | (2 << 62))
}
pub(crate) trait Access {
    fn active(&self) -> Result<String>;
    fn exists(&self, id: &str) -> Result<bool>;
    fn import(&self, id: &str) -> Result<()>;
    fn verify(&self, id: &str) -> Result<()>;
    fn activate(&self, id: &str) -> Result<()>;
    fn delete(&self, id: &str) -> Result<()>;
    fn cleanup(&self) -> Result<()>;
}
pub(crate) fn enter(io: &impl Access, before: &str, target: &str) -> Result<()> {
    if io.active()? != before {
        bail!("活动电源计划已变化，未导入校准计划");
    }
    io.import(target)?;
    io.verify(target)?;
    io.activate(target)?;
    check(io, target)
}
pub(crate) fn prepare(io: &impl Access, before: &str, target: &str) -> Result<()> {
    parse_guid(before)?;
    parse_guid(target)?;
    if before == target || io.active()? != before {
        bail!("活动电源计划已变化，请重新预览");
    }
    if io.exists(target)? {
        bail!("本次校准计划标识已被占用，未覆盖现有计划");
    }
    Ok(())
}
pub(crate) fn check(io: &impl Access, target: &str) -> Result<()> {
    if io.active()? != target {
        bail!("校准电源计划已被外部切换");
    }
    io.verify(target)
}
pub(crate) fn restore(io: &impl Access, before: &str, target: &str) -> Result<()> {
    parse_guid(before)?;
    parse_guid(target)?;
    if before == target {
        bail!("校准恢复计划与临时计划不能相同");
    }
    let mut errors = Vec::new();
    let restored = io.active().and_then(|active| {
        if active != before {
            io.activate(before)?;
        }
        if io.active()? != before {
            bail!("原电源计划恢复回读不一致");
        }
        Ok(())
    });
    if let Err(error) = restored {
        errors.push(format!("恢复活动计划：{error:#}"));
    } else if let Err(error) = io
        .exists(target)
        .and_then(|present| if present { io.delete(target) } else { Ok(()) })
    {
        errors.push(format!("删除校准临时计划：{error:#}"));
    }
    if let Err(error) = io.cleanup() {
        errors.push(format!("清理校准资源：{error:#}"));
    }
    if !errors.is_empty() {
        bail!(errors.join("；"));
    }
    Ok(())
}
