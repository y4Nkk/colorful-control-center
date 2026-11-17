use super::Hal;
use anyhow::{Result, bail};
use serde::Serialize;

#[cfg(not(test))]
mod native;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Reading {
    pub panel: String,
    pub current: u8,
    pub on_ac: bool,
}
pub(super) fn decode_supply(value: u8) -> Result<bool> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => bail!("当前供电状态未知"),
    }
}
impl Reading {
    pub(crate) fn validate(&self) -> Result<()> {
        if self.panel.is_empty() || self.panel.len() > 4096 || self.current > 100 {
            bail!("活动面板亮度报告无效");
        }
        Ok(())
    }
}
impl Hal {
    pub(crate) fn screen_brightness(&self) -> Result<Reading> {
        #[cfg(test)]
        let reading = self
            .test_screen_brightness
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("隔离测试未注入屏幕亮度报告"))?()?;
        #[cfg(not(test))]
        let reading = native::read()?;
        reading.validate()?;
        Ok(reading)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_documented_ac_and_dc_supply_reports_are_accepted() {
        for value in 0..=u8::MAX {
            match value {
                0 => assert!(!decode_supply(value).unwrap()),
                1 => assert!(decode_supply(value).unwrap()),
                _ => assert!(decode_supply(value).is_err()),
            }
        }
    }
    #[test]
    fn panel_identity_and_current_percentage_must_be_explicit_and_in_range() {
        let mut reading = Reading {
            panel: "panel".into(),
            current: 0,
            on_ac: true,
        };
        assert!(reading.validate().is_ok());
        reading.current = 100;
        assert!(reading.validate().is_ok());
        reading.current = 101;
        assert!(reading.validate().is_err());
        reading.current = 50;
        reading.panel.clear();
        assert!(reading.validate().is_err());
        reading.panel = "x".repeat(4097);
        assert!(reading.validate().is_err());
    }
}
