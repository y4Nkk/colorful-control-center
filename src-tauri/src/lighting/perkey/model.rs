use super::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum Pattern {
    Static { colors: Vec<Rgb> },
    Wave { direction: u8, color: Option<Rgb> },
    Breath { color: Option<Rgb> },
    Blink { color: Option<Rgb> },
    Random { color: Option<Rgb> },
    Scan { colors: Option<[Rgb; 2]> },
    Snake { direction: u8, color: Option<Rgb> },
    Ripple { reactive: bool, color: Option<Rgb> },
    Fn { color: Rgb },
    Fade { color: Option<Rgb> },
}
impl Pattern {
    pub fn reactive(&self) -> bool {
        matches!(
            self,
            Self::Ripple { .. } | Self::Fn { .. } | Self::Fade { .. }
        )
    }
    fn validate(&self, device: &device::DeviceInfo) -> Result<()> {
        self.validate_shape()?;
        match self {
            Self::Wave { direction, color }
                if !device.new_effects && (*direction != 0 || color.is_some()) =>
            {
                anyhow::bail!("本MCU不支持所选波浪方向或颜色")
            }
            Self::Snake { direction, color }
                if !device.new_effects && (*direction != 0 || color.is_some()) =>
            {
                anyhow::bail!("本MCU不支持所选蛇形方向或颜色")
            }
            Self::Random { color: Some(_) } | Self::Scan { colors: Some(_) }
                if !device.new_effects =>
            {
                anyhow::bail!("本MCU未声明自定义随机或扫描效果")
            }
            _ => (),
        }
        Ok(())
    }
    fn validate_shape(&self) -> Result<()> {
        match self {
            Self::Static { colors } if colors.len() != 120 => {
                anyhow::bail!("逐键静态目标必须包含120个颜色")
            }
            Self::Wave { direction, .. } if *direction > 7 => anyhow::bail!("逐键波浪方向超出范围"),
            Self::Snake { direction, .. } if *direction > 3 => {
                anyhow::bail!("逐键蛇形方向超出范围")
            }
            _ => (),
        }
        let black = |c: &Rgb| c.r == 0 && c.g == 0 && c.b == 0;
        let color = match self {
            Self::Wave { color, .. }
            | Self::Breath { color }
            | Self::Blink { color }
            | Self::Random { color }
            | Self::Snake { color, .. }
            | Self::Ripple { color, .. }
            | Self::Fade { color } => color.as_ref(),
            Self::Fn { color } => Some(color),
            _ => None,
        };
        if color.is_some_and(black)
            || matches!(self, Self::Scan { colors: Some(c) } if c.iter().all(black))
        {
            anyhow::bail!("此动态效果的自定义颜色不能为黑色；可关闭灯光");
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Values {
    pub brightness: u8,
    pub enabled: bool,
    pub speed: u8,
    pub pattern: Pattern,
}
impl Values {
    pub fn validate(&self) -> Result<()> {
        if self.brightness > 4 || self.speed > 2 {
            anyhow::bail!("逐键亮度为0–4级，速度编码为0–2");
        }
        self.pattern.validate_shape()
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Target {
    pub keyboard_type: u8,
    pub device: device::DeviceInfo,
    pub values: Values,
}
impl Target {
    pub fn validate(&self) -> Result<()> {
        if family_from_type(self.keyboard_type) != KeyboardFamily::Perkey
            || self.device.language > 15
        {
            anyhow::bail!("逐键档案型号或布局信息无效");
        }
        self.values.validate()?;
        self.values.pattern.validate(&self.device)
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub expected_revision: String,
    pub values: Values,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub revision: String,
    pub current: Values,
    pub device: device::DeviceInfo,
    pub presets: Vec<Pattern>,
    pub can_restore: bool,
    pub reactive_error: Option<String>,
    pub reactive_running: bool,
}
pub(super) fn target(
    before: &Snapshot,
    request: &Values,
    info: &device::DeviceInfo,
) -> Result<Snapshot> {
    request.validate()?;
    request.pattern.validate(info)?;
    let mut s = before.clone();
    s.put(
        33,
        u8::from(matches!(request.pattern, Pattern::Static { .. })),
    );
    s.put(34, request.speed);
    s.put(35, request.brightness);
    s.put(80, u8::from(request.enabled));
    apply_pattern(&mut s, &request.pattern, info)?;
    Ok(s)
}
fn apply_pattern(s: &mut Snapshot, pattern: &Pattern, device: &device::DeviceInfo) -> Result<()> {
    let custom = |s: &mut Snapshot, at: usize, color: &Option<Rgb>| {
        s.put(at, u8::from(color.is_some()));
        if let Some(c) = color {
            s.color(at + 1, *c);
        }
    };
    let mode = match pattern {
        Pattern::Static { colors } => {
            for (i, c) in colors.iter().enumerate() {
                let at = matrix_offset(i);
                s.matrix[at..at + 3].copy_from_slice(&[c.r, c.g, c.b]);
            }
            return Ok(());
        }
        Pattern::Wave { direction, color } => {
            s.put(64, *direction + if color.is_some() { 8 } else { 0 });
            if let Some(c) = color {
                s.color(66, *c);
            }
            if device.new_effects { 7 } else { 0 }
        }
        Pattern::Breath { color } => {
            custom(s, 52, color);
            1
        }
        Pattern::Blink { color } => {
            custom(s, 56, color);
            2
        }
        Pattern::Random { color } => {
            custom(s, 76, color);
            if device.new_effects { 9 } else { 3 }
        }
        Pattern::Scan { colors } => {
            s.put(69, u8::from(colors.is_some()));
            if let Some(c) = colors {
                s.color(70, c[0]);
                s.color(73, c[1]);
            }
            if device.new_effects { 8 } else { 4 }
        }
        Pattern::Snake { direction, color } => {
            s.put(60, *direction + if color.is_some() { 4 } else { 0 });
            if let Some(c) = color {
                s.color(61, *c);
            }
            if device.new_effects { 10 } else { 6 }
        }
        Pattern::Ripple { reactive, color } => {
            s.put(48, u8::from(*reactive) * 2 + u8::from(color.is_some()));
            if let Some(c) = color {
                s.color(49, *c);
            }
            5
        }
        Pattern::Fn { color } => {
            s.color(45, *color);
            11
        }
        Pattern::Fade { color } => {
            custom(s, 41, color);
            13
        }
    };
    s.put(32, mode);
    Ok(())
}
