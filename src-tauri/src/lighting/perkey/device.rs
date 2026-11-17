use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeviceInfo {
    pub new_effects: bool,
    pub language: u8,
    pub narrow_border: bool,
    pub three_column: bool,
    pub compact: Option<bool>,
    pub physical_three_column: Option<bool>,
}
pub fn query(selector: u8) -> Result<[u8; 16]> {
    if ![1, 16].contains(&selector) {
        bail!("逐键查询选择器未开放");
    }
    let mut bytes = [0; 16];
    bytes[..3].copy_from_slice(&[0xCC, 17, selector]);
    Ok(bytes)
}
pub fn payload(bytes: &[u8; 16]) -> Result<[u8; 3]> {
    if bytes[0] != 0xCC {
        bail!("逐键MCU响应报告编号不匹配");
    }
    Ok(bytes[2..5].try_into().expect("固定报告长度"))
}
pub fn decode(layout: [u8; 3], features: [u8; 3]) -> DeviceInfo {
    DeviceInfo {
        new_effects: features[0] & 1 != 0,
        language: layout[0] & 15,
        narrow_border: layout[0] & 16 != 0,
        three_column: layout[0] & 32 != 0,
        compact: None,
        physical_three_column: None,
    }
}
pub fn compact(packet: &[u8; 256]) -> Option<bool> {
    let width = u16::from(packet[66]) | (u16::from(packet[68] & 0xF0) << 4);
    let height = u16::from(packet[67]) | (u16::from(packet[68] & 15) << 8);
    if width == 0 || height == 0 {
        return None;
    }
    Some(f64::from(width).hypot(f64::from(height)) / 25.4 < 15.0)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_queries_and_original_response_offsets_do_not_clear_lights() {
        for selector in [1, 16] {
            let r = query(selector).unwrap();
            assert_eq!(&r[..3], &[0xCC, 17, selector]);
            assert!(r[3..].iter().all(|v| *v == 0));
        }
        assert!(query(0).is_err());
        let mut raw = [0; 16];
        raw[..5].copy_from_slice(&[0xCC, 17, 0x31, 2, 3]);
        assert_eq!(payload(&raw).unwrap(), [0x31, 2, 3]);
        raw[0] = 0;
        assert!(payload(&raw).is_err());
        let info = decode([0x31, 0, 0], [1, 0, 0]);
        assert!(info.new_effects && info.narrow_border && info.three_column);
        assert_eq!(info.language, 1);
    }
    #[test]
    fn panel_geometry_uses_both_edid_dimensions_and_preserves_unknown() {
        assert_eq!(compact(&[0; 256]), None);
        let mut packet = [0; 256];
        packet[66] = 0x35;
        packet[67] = 0xC2;
        packet[68] = 0x10;
        assert_eq!(compact(&packet), Some(true));
        packet[66] = 0x58;
        packet[67] = 0xD7;
        assert_eq!(compact(&packet), Some(false));
        packet[67] = 0;
        assert_eq!(compact(&packet), None);
    }
}
