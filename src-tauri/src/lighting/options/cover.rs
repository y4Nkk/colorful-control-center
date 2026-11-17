use super::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TopCover {
    pub brightness: u8,
    pub color: Rgb,
}
impl TopCover {
    pub fn validate(&self) -> Result<()> {
        if self.brightness > 4 {
            bail!("独立顶盖亮度须为0–4级");
        }
        Ok(())
    }
}
pub(super) fn commands(value: &TopCover) -> Result<Vec<(i32, u32)>> {
    value.validate()?;
    let mut words = vec![(103, encode_rgb_brightness_word(value.brightness, None)?)];
    let color = value.color;
    let blue = if color
        == (Rgb {
            r: 0,
            g: 254,
            b: 129,
        })
        || color
            == (Rgb {
                r: 0,
                g: 255,
                b: 127,
            }) {
        70
    } else {
        color.b
    };
    let packed = u32::from(blue) << 16 | u32::from(color.r) << 8 | u32::from(color.g);
    words.extend([0xF0, 0xF1, 0xF2].map(|part| (103, (part << 24) | packed)));
    Ok(words)
}
