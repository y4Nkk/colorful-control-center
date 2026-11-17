use super::Hal;
use anyhow::{Result, bail};
use serde::Serialize;
use sha2::{Digest, Sha256};

#[cfg(not(test))]
mod native;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Monitor {
    pub instance: String,
    pub name: String,
    pub hardware_ids: Vec<String>,
    pub signature: String,
    pub fingerprint: String,
    pub oled: bool,
    pub status: u32,
    pub problem: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct Inventory {
    pub monitors: Vec<Monitor>,
    pub unresolved: Vec<String>,
}
fn decode_property(buffer: &[u16], multi: bool) -> Result<Vec<String>> {
    let text = if multi {
        if buffer.len() < 3 || buffer[buffer.len() - 2..] != [0, 0] {
            bail!("面板硬件ID缺少多字符串结束符");
        }
        &buffer[..buffer.len() - 2]
    } else {
        if buffer.len() < 2 || buffer.last() != Some(&0) {
            bail!("面板设备属性缺少结束符");
        }
        &buffer[..buffer.len() - 1]
    };
    let mut values = Vec::new();
    for part in text.split(|value| *value == 0) {
        if part.is_empty() || !multi && !values.is_empty() {
            bail!("面板设备属性包含空值或多余字符串");
        }
        values.push(String::from_utf16(part)?);
    }
    Ok(values)
}
impl Monitor {
    pub(crate) fn from_edid(
        instance: String,
        name: String,
        mut hardware_ids: Vec<String>,
        edid: &[u8],
        status: u32,
        problem: u32,
    ) -> Result<Self> {
        let (signature, oled) =
            ccc_core::panel_hotkey::edid_signature(edid).map_err(anyhow::Error::msg)?;
        hardware_ids.sort();
        hardware_ids.dedup();
        let monitor = Self {
            instance,
            name,
            hardware_ids,
            signature,
            oled,
            status,
            problem,
            fingerprint: format!("{:x}", Sha256::digest(edid)),
        };
        monitor.validate()?;
        Ok(monitor)
    }
    fn validate(&self) -> Result<()> {
        if self.instance.is_empty()
            || self.instance.len() > 4096
            || self.name.is_empty()
            || self.name.len() > 4096
            || self.hardware_ids.is_empty()
            || self.hardware_ids.len() > 64
            || self
                .hardware_ids
                .iter()
                .any(|id| id.is_empty() || id.len() > 4096 || id.contains('\0'))
            || self.signature.len() != 8
            || !self.signature.bytes().all(|b| b.is_ascii_hexdigit())
            || self.fingerprint.len() != 64
            || !self.fingerprint.bytes().all(|b| b.is_ascii_hexdigit())
            || self.oled != ["4C8329A0", "4C834341"].contains(&self.signature.as_str())
        {
            bail!("当前显示面板身份或EDID报告无效");
        }
        Ok(())
    }
}
impl Hal {
    pub(crate) fn panel_inventory(&self) -> Result<Inventory> {
        #[cfg(test)]
        let mut inventory =
            self.test_panel_inventory
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("隔离测试未注入显示面板清单"))?()?;
        #[cfg(not(test))]
        let mut inventory = native::read()?;
        if inventory.monitors.len() + inventory.unresolved.len() > 256
            || inventory.monitors.is_empty() && inventory.unresolved.is_empty()
            || inventory
                .unresolved
                .iter()
                .any(|e| e.is_empty() || e.len() > 8192)
        {
            bail!("当前显示面板清单为空或超出数量范围");
        }
        for monitor in &inventory.monitors {
            monitor.validate()?;
        }
        inventory
            .monitors
            .sort_by(|a, b| a.instance.cmp(&b.instance));
        inventory.unresolved.sort();
        if inventory
            .monitors
            .windows(2)
            .any(|pair| pair[0].instance == pair[1].instance)
        {
            bail!("显示面板清单包含重复实例");
        }
        Ok(inventory)
    }
}

#[cfg(test)]
mod tests {
    use super::decode_property;

    #[test]
    fn device_strings_reject_truncation_empty_segments_and_embedded_extra_values() {
        assert_eq!(decode_property(&[65, 0], false).unwrap(), ["A"]);
        assert_eq!(
            decode_property(&[65, 0, 66, 0, 0], true).unwrap(),
            ["A", "B"]
        );
        for invalid in [&[][..], &[0][..], &[65][..], &[65, 0, 66, 0][..]] {
            assert!(decode_property(invalid, false).is_err());
        }
        for invalid in [
            &[][..],
            &[0, 0][..],
            &[65, 0][..],
            &[65, 0, 0, 66, 0, 0][..],
        ] {
            assert!(decode_property(invalid, true).is_err());
        }
        assert!(decode_property(&[0xd800, 0], false).is_err());
    }
}
