//! 后台档案许可只绑定完整文件和稳定模式意图，不保存单次传感器前态。
use super::{ProfileFile, load_from, sanitize_name, validate_file};
use crate::powerctl::mode::profile::Grant;
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Binding {
    pub profile: String,
    pub digest: String,
    #[serde(deserialize_with = "Option::deserialize")]
    pub mode: Option<Grant>,
}
impl Binding {
    pub(crate) fn reviewed(file: &ProfileFile, mode: Option<Grant>) -> Result<Self> {
        validate_file(file)?;
        if file.settings.oem_power_mode.is_some() != mode.is_some() {
            bail!("后台档案模式尚未获得完整授权预览");
        }
        let binding = Self {
            profile: file.name.clone(),
            digest: digest(file)?,
            mode,
        };
        binding.validate()?;
        Ok(binding)
    }
    pub(crate) fn validate(&self) -> Result<()> {
        if sanitize_name(&self.profile)? != self.profile
            || self.digest.len() != 64
            || !self
                .digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            bail!("后台档案确认记录无效，请重新选择并确认");
        }
        if let Some(mode) = &self.mode {
            mode.validate()?;
        }
        Ok(())
    }
    pub(crate) fn load(&self, dir: &Path) -> Result<ProfileFile> {
        self.validate()?;
        let file = load_from(dir, &self.profile)?;
        if digest(&file)? != self.digest
            || file.settings.oem_power_mode.is_some() != self.mode.is_some()
        {
            bail!("档案内容已变化，请重新预览并确认后台规则");
        }
        Ok(file)
    }
    pub(crate) fn bind_driver(&mut self, identity: &str) -> Result<bool> {
        self.mode
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("档案没有GPU模式授权"))?
            .bind_driver(identity)
    }
}
fn digest(file: &ProfileFile) -> Result<String> {
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(file)?)))
}
