use super::*;

pub(in crate::lighting) fn capabilities(dchu: &Dchu) -> Result<Capabilities> {
    let mut keyboard = dchu.read_package(13)?;
    let cached = dchu.read_settings(2, 4, 1)?.first().copied();
    keyboard[15] = super::super::select_detection(Some(keyboard[15]), cached)
        .0
        .context("键盘类型未知")?;
    let flags = dchu.read_bios_capabilities(0, 24)?;
    decode(&keyboard, &flags)
}
pub(super) fn decode(keyboard: &[u8; 256], flags: &[u8]) -> Result<Capabilities> {
    if flags.len() != 24 {
        bail!("灯光扩展能力页长度无效");
    }
    let keyboard_type = keyboard[15];
    let family = family_from_type(keyboard_type);
    if !matches!(
        family,
        KeyboardFamily::ZoneRgb
            | KeyboardFamily::White
            | KeyboardFamily::Perkey
            | KeyboardFamily::MultiZone
    ) {
        bail!("固件未声明支持的背光键盘类型");
    }
    let known = match &flags[..2] {
        [1, 0] => true,
        [0, 0] => false,
        _ => bail!("未知灯光扩展能力页版本"),
    };
    let wmi122 = u32::from_le_bytes(flags[6..10].try_into()?);
    let top_cover = known && flags[23] & 64 != 0;
    Ok(Capabilities {
        keyboard_type,
        top_cover,
        peripheral_support_known: known,
        boot_override: keyboard[43] & 1 != 0,
        logo: known && !top_cover && family == KeyboardFamily::ZoneRgb && wmi122 & (1 << 18) != 0,
        light_bar: known && family == KeyboardFamily::ZoneRgb && wmi122 & (1 << 12) != 0,
    })
}
impl Io for Dchu {
    fn capabilities(&self) -> Result<Capabilities> {
        capabilities(self)
    }
    fn send(&self, command: i32, word: u32) -> Result<()> {
        self.set_u32(command, word)
    }
}
