//! 可交换配置只含完整保存意图；导入预览不初始化控制器或访问驱动。
use anyhow::{Context, Result, anyhow, bail};
use ccc_core::keymap::{self, Configuration};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct File {
    version: u8,
    name: String,
    configuration: Configuration,
}
pub fn preview(json: &str) -> Result<File> {
    if json.len() > 4 * 1024 * 1024 {
        bail!("映射配置不能超过4 MB");
    }
    let file: File = serde_json::from_str(json).context("映射配置格式无效")?;
    if file.version != 1 {
        bail!("未知映射配置版本");
    }
    if file.name.trim().is_empty()
        || file.name.chars().count() > 80
        || file.name.chars().any(char::is_control)
    {
        bail!("映射配置名称须为1–80个可显示字符");
    }
    keymap::validate(&file.configuration).map_err(|e| anyhow!(e))?;
    if file.configuration.devices.len() != 2 {
        bail!("映射配置须同时包含键盘与鼠标");
    }
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    const CONFIG: &str = r#"{"version":1,"name":"工作","configuration":{"devices":[{"device":"keyboard","enabled":true,"bindings":[]},{"device":"mouse","enabled":false,"bindings":[]}]}}"#;
    #[test]
    fn full_file_roundtrips_and_never_needs_a_hardware_provider() {
        let file = preview(CONFIG).unwrap();
        assert_eq!(file.name, "工作");
        assert_eq!(
            preview(&serde_json::to_string(&file).unwrap())
                .unwrap()
                .configuration,
            file.configuration
        );
    }
    #[test]
    fn rejects_old_versions_unknown_fields_incomplete_devices_and_unbalanced_macros() {
        assert!(preview(&CONFIG.replace("\"version\":1", "\"version\":0")).is_err());
        assert!(preview(&CONFIG.replace("\"version\":1", "\"extra\":0,\"version\":1")).is_err());
        assert!(
            preview(&CONFIG.replace(
                ",{\"device\":\"mouse\",\"enabled\":false,\"bindings\":[]}",
                ""
            ))
            .is_err()
        );
        let bad = CONFIG.replacen("\"bindings\":[]", r#""bindings":[{"trigger":30,"action":{"kind":"keys","steps":[{"kind":"key","scanCode":30,"pressed":true,"delayAfterMs":0}]}}]"#, 1);
        assert!(preview(&bad).is_err());
    }
}
