use super::*;
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Effect {
    Static,
    Breath,
    Wave,
    Cycle,
    Scan,
    ColorWave,
}
pub const EFFECTS: [Effect; 6] = [
    Effect::Static,
    Effect::Breath,
    Effect::Wave,
    Effect::Cycle,
    Effect::Scan,
    Effect::ColorWave,
];
impl Effect {
    fn index(self) -> u8 {
        match self {
            Self::Static => 0,
            Self::Breath => 1,
            Self::Wave => 2,
            Self::Cycle => 3,
            Self::Scan => 4,
            Self::ColorWave => 5,
        }
    }
    fn color_at(self) -> Option<usize> {
        match self {
            Self::Static => Some(3),
            Self::Scan => Some(4),
            _ => None,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Values {
    pub effect: Effect,
    pub speed: u8,
    pub brightness: u8,
    pub color_id: Option<u8>,
}
impl Values {
    pub fn validate(&self) -> Result<()> {
        if !(1..=3).contains(&self.speed) || self.brightness > 4 {
            bail!("X58速度为1–3档，亮度为0–4级");
        }
        if self.effect.color_at().is_some() != self.color_id.is_some()
            || self.color_id.is_some_and(|id| !(1..=7).contains(&id))
        {
            bail!("X58颜色只能为当前模式的七种原厂颜色");
        }
        Ok(())
    }
}
pub fn for_effect(bytes: &[u8; 19], effect: Effect) -> Values {
    Values {
        effect,
        speed: bytes[1],
        brightness: bytes[2],
        color_id: effect.color_at().map(|index| bytes[index]),
    }
}
pub fn values(bytes: &[u8; 19]) -> Result<Values> {
    let effect = EFFECTS
        .get(usize::from(bytes[0]))
        .copied()
        .context("X58保存模式未知或尚未初始化")?;
    let result = for_effect(bytes, effect);
    result.validate()?;
    Ok(result)
}
pub fn target(bytes: &[u8; 19], value: &Values) -> Result<[u8; 19]> {
    values(bytes)?;
    value.validate()?;
    let mut next = *bytes;
    next[..3].copy_from_slice(&[value.effect.index(), value.speed, value.brightness]);
    if let Some(index) = value.effect.color_at() {
        next[index] = value.color_id.context("X58模式缺少颜色")?;
    }
    Ok(next)
}
fn packet(payload: &[u8]) -> [u8; 64] {
    let mut result = [0; 64];
    result[0] = model::Kind::X58.report_id();
    result[1] = 0xe2;
    result[2..payload.len() + 2].copy_from_slice(payload);
    result
}
pub fn packets(bytes: &[u8; 19]) -> Result<Vec<[u8; 64]>> {
    let value = values(bytes)?;
    let mut result = vec![
        packet(&[1, value.speed]),
        packet(&[value.effect.index() + 5]),
    ];
    if let Some(color) = value.color_id {
        result.push(packet(&[3, color]));
    }
    result.push(packet(&[2, value.brightness + 1]));
    Ok(result)
}
pub fn supported(status: u8) -> Result<bool> {
    match status {
        0 => Ok(false),
        1 => Ok(true),
        _ => bail!("固件报告未知的新灯条通道，未绑定X58"),
    }
}
