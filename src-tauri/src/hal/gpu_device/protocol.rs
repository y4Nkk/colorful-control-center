use anyhow::{Result, bail};

pub fn supported(caps: &[u8]) -> Result<bool> {
    if caps.len() < 20 {
        bail!("原厂能力页长度不足");
    }
    if caps[..2] != [0, 0] && caps[..2] != [1, 0] {
        bail!("未知原厂能力页版本，未开放独显控制");
    }
    Ok(caps[..2] == [1, 0] && caps[19] & 0x40 != 0)
}
pub fn automatic(raw: u8) -> Result<bool> {
    match raw {
        0 => Ok(true),
        1 => Ok(false),
        _ => anyhow::bail!("未知原厂自动关断策略，不能保证恢复"),
    }
}
pub fn firmware_packet(enabled: bool) -> [u8; 256] {
    let mut packet = [0; 256];
    packet[0] = 34;
    packet[1] = u8::from(enabled);
    packet
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_confirmed_extended_capability_opens_oem_power() {
        let mut caps = [0; 20];
        caps[19] = 0x40;
        assert!(!supported(&caps).unwrap());
        caps[0] = 1;
        assert!(supported(&caps).unwrap());
        caps[19] = 0;
        assert!(!supported(&caps).unwrap());
        caps[1] = 1;
        assert!(supported(&caps).is_err());
        assert!(supported(&caps[..19]).is_err());
    }
    #[test]
    fn policy_and_full_firmware_packet_match_original() {
        assert!(automatic(0).unwrap());
        assert!(!automatic(1).unwrap());
        assert!(automatic(2).is_err());
        for enabled in [false, true] {
            let packet = firmware_packet(enabled);
            assert_eq!(&packet[..2], &[34, u8::from(enabled)]);
            assert!(packet[2..].iter().all(|v| *v == 0));
        }
    }
}
