use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    BlankSaver,
    PointerWake,
}

pub fn edid_signature(bytes: &[u8]) -> Result<(String, bool), String> {
    if bytes.len() < 128
        || bytes.len() > 4096
        || bytes[..8] != [0, 255, 255, 255, 255, 255, 255, 0]
        || bytes[..128]
            .iter()
            .fold(0u8, |sum, value| sum.wrapping_add(*value))
            != 0
    {
        return Err("当前面板EDID不完整或校验失败".into());
    }
    let signature: [u8; 4] = bytes[8..12].try_into().unwrap();
    let oled = [[0x4C, 0x83, 0x29, 0xA0], [0x4C, 0x83, 0x43, 0x41]].contains(&signature);
    Ok((
        signature.iter().map(|byte| format!("{byte:02X}")).collect(),
        oled,
    ))
}
pub fn pointer_wake_allowed(flags: &[u8]) -> Result<bool, String> {
    match flags.get(..2) {
        Some([0, 0]) => Ok(true),
        Some([1, 0]) => flags
            .get(15)
            .map(|value| value & 8 == 0)
            .ok_or_else(|| "屏幕按键能力页不完整".into()),
        _ => Err("屏幕按键能力页版本未知".into()),
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Activity {
    KeyDown,
    Pointer,
}
pub fn requests_wake(activity: Activity) -> bool {
    activity == Activity::Pointer
}

#[cfg(test)]
mod tests {
    use super::*;
    fn edid(signature: [u8; 4]) -> [u8; 128] {
        let mut bytes = [0; 128];
        bytes[..8].copy_from_slice(&[0, 255, 255, 255, 255, 255, 255, 0]);
        bytes[8..12].copy_from_slice(&signature);
        bytes[127] = 0u8.wrapping_sub(bytes[..127].iter().fold(0u8, |sum, v| sum.wrapping_add(*v)));
        bytes
    }
    #[test]
    fn oled_requires_either_exact_original_signature_and_valid_base_edid() {
        for signature in [[0x4C, 0x83, 0x29, 0xA0], [0x4C, 0x83, 0x43, 0x41]] {
            assert!(edid_signature(&edid(signature)).unwrap().1);
            let mut other = signature;
            other[3] ^= 1;
            assert!(!edid_signature(&edid(other)).unwrap().1);
        }
        let mut bytes = edid([0x4C, 0x83, 0x29, 0xA0]);
        assert_eq!(edid_signature(&bytes).unwrap().0, "4C8329A0");
        assert!(edid_signature(&bytes[..127]).is_err());
        bytes[13] ^= 1;
        assert!(edid_signature(&bytes).is_err());
        bytes[13] ^= 1;
        bytes[0] ^= 1;
        assert!(edid_signature(&bytes).is_err());
    }
    #[test]
    fn declared_versions_decode_the_hook_flag_without_guessing_unknown_layouts() {
        assert!(pointer_wake_allowed(&[0, 0]).unwrap());
        let mut flags = [0; 16];
        flags[0] = 1;
        assert!(pointer_wake_allowed(&flags).unwrap());
        flags[15] = 8;
        assert!(!pointer_wake_allowed(&flags).unwrap());
        assert!(pointer_wake_allowed(&flags[..15]).is_err());
        flags[0] = 2;
        assert!(pointer_wake_allowed(&flags).is_err());
        assert!(pointer_wake_allowed(&[]).is_err());
    }
    #[test]
    fn only_pointer_activity_requests_wake() {
        assert!(!requests_wake(Activity::KeyDown));
        assert!(requests_wake(Activity::Pointer));
    }
}
