use super::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    X170,
    X58,
}
impl Kind {
    pub fn product(self) -> u16 {
        match self {
            Self::X170 => 0x8297,
            Self::X58 => 0x8911,
        }
    }
    pub fn report_id(self) -> u8 {
        match self {
            Self::X170 => 0xcc,
            Self::X58 => 0xcd,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Effect {
    Static,
    Breath,
    Blink,
    Cycle,
    Meteor,
    Wave,
    Ripple,
    Random,
}
pub const EFFECTS: [Effect; 8] = [
    Effect::Static,
    Effect::Breath,
    Effect::Blink,
    Effect::Cycle,
    Effect::Meteor,
    Effect::Wave,
    Effect::Ripple,
    Effect::Random,
];
impl Effect {
    pub fn command(self) -> u8 {
        match self {
            Self::Static => 176,
            Self::Breath => 178,
            Self::Blink => 179,
            Self::Cycle => 180,
            Self::Meteor => 181,
            Self::Wave => 182,
            Self::Ripple => 183,
            Self::Random => 184,
        }
    }
    fn offset(self) -> Option<usize> {
        match self {
            Self::Static => Some(4),
            Self::Breath => Some(8),
            Self::Blink => Some(12),
            Self::Meteor => Some(16),
            _ => None,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct X170 {
    pub enabled: bool,
    pub brightness: u8,
    pub speed: u8,
    pub effect: Effect,
    pub color: Option<Rgb>,
}
impl X170 {
    pub fn validate(&self) -> Result<()> {
        if self.brightness > 4 || self.speed > 2 {
            bail!("灯条亮度为0–4级，速度为0–2");
        }
        if self.effect == Effect::Static && self.color.is_none()
            || self.effect.offset().is_none() && self.color.is_some()
        {
            bail!("此灯条模式的颜色选项无效");
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "settings",
    rename_all = "camelCase",
    deny_unknown_fields
)]
pub enum Values {
    X170(X170),
    X58(super::x58::Values),
}
impl Values {
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::X170(value) => value.validate(),
            Self::X58(value) => value.validate(),
        }
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
    pub kind: Kind,
    pub revision: String,
    pub current: Option<Values>,
    pub presets: Vec<Values>,
    pub initial: Option<Values>,
    pub blocked_reason: Option<String>,
    pub warnings: Vec<String>,
}
#[derive(Clone)]
pub(crate) struct Snapshot {
    pub kind: Kind,
    pub bytes: [u8; 19],
    pub identity: String,
}
impl Snapshot {
    pub fn revision(&self) -> String {
        format!(
            "{:x}",
            Sha256::digest(
                [
                    &[self.kind.report_id()][..],
                    self.identity.as_bytes(),
                    &self.bytes
                ]
                .concat()
            )
        )
    }
    pub fn values(&self) -> Result<Values> {
        match self.kind {
            Kind::X170 => self.x170_values().map(Values::X170),
            Kind::X58 => super::x58::values(&self.bytes).map(Values::X58),
        }
    }
    fn x170_values(&self) -> Result<X170> {
        let effect = EFFECTS
            .iter()
            .find(|e| e.command() == self.bytes[0])
            .copied()
            .context("灯条保存模式未知或尚未初始化")?;
        if self.bytes[3] > 1 || [7, 11, 15].iter().any(|i| self.bytes[*i] > 1) {
            bail!("灯条保存开关或颜色模式未知");
        }
        let result = self.for_effect(effect);
        result.validate()?;
        Ok(result)
    }
    pub fn for_effect(&self, effect: Effect) -> X170 {
        let color = effect
            .offset()
            .filter(|offset| effect == Effect::Static || self.bytes[offset - 1] == 1)
            .map(|at| Rgb {
                r: self.bytes[at],
                g: self.bytes[at + 1],
                b: self.bytes[at + 2],
            });
        X170 {
            enabled: self.bytes[3] == 1,
            brightness: self.bytes[2],
            speed: self.bytes[1],
            effect,
            color,
        }
    }
    pub fn target(&self, values: &Values) -> Result<Self> {
        self.values()?;
        match (self.kind, values) {
            (Kind::X170, Values::X170(values)) => self.target_x170(values),
            (Kind::X58, Values::X58(values)) => {
                let mut next = self.clone();
                next.bytes = super::x58::target(&self.bytes, values)?;
                Ok(next)
            }
            _ => bail!("灯条目标型号与实际设备不一致"),
        }
    }
    fn target_x170(&self, values: &X170) -> Result<Self> {
        values.validate()?;
        let mut next = self.clone();
        next.bytes[..4].copy_from_slice(&[
            values.effect.command(),
            values.speed,
            values.brightness,
            u8::from(values.enabled),
        ]);
        if let Some(at) = values.effect.offset() {
            if values.effect != Effect::Static {
                next.bytes[at - 1] = u8::from(values.color.is_some());
            }
            if let Some(color) = values.color {
                next.bytes[at..at + 3].copy_from_slice(&[color.r, color.g, color.b]);
            }
        }
        Ok(next)
    }
    pub fn packets(&self) -> Result<Vec<[u8; 64]>> {
        if self.kind == Kind::X58 {
            return super::x58::packets(&self.bytes);
        }
        let values = self.x170_values()?;
        let mut data = [0; 62];
        data[0] = u8::from(values.color.is_some());
        data[1] = values.speed;
        if let Some(color) = values.color {
            data[2..5].copy_from_slice(&[color.r, color.g, color.b]);
        }
        Ok(vec![
            brightness(0),
            report(values.effect.command(), data),
            brightness(if values.enabled { values.brightness } else { 0 }),
        ])
    }
    pub fn presets(&self) -> Vec<Values> {
        match self.kind {
            Kind::X170 => EFFECTS
                .iter()
                .map(|effect| Values::X170(self.for_effect(*effect)))
                .collect(),
            Kind::X58 => super::x58::EFFECTS
                .iter()
                .filter_map(|effect| {
                    let value = super::x58::for_effect(&self.bytes, *effect);
                    value.validate().ok().map(|_| Values::X58(value))
                })
                .collect(),
        }
    }
    pub fn same_device(&self, other: &Self) -> bool {
        self.kind == other.kind && self.identity == other.identity
    }
}
pub fn report(command: u8, data: [u8; 62]) -> [u8; 64] {
    let mut report = [0; 64];
    report[0] = 0xcc;
    report[1] = command;
    report[2..].copy_from_slice(&data);
    report
}
fn brightness(value: u8) -> [u8; 64] {
    let mut data = [0; 62];
    data[0] = value;
    report(191, data)
}
pub fn kinds(flags: &[u8]) -> Result<Vec<Kind>> {
    if flags.len() != 24 {
        bail!("独立灯条能力页长度无效");
    }
    match &flags[..2] {
        [0, 0] => Ok(vec![]),
        [1, 0] => Ok([
            (flags[11] & 8 != 0, Kind::X170),
            (flags[23] & 128 != 0, Kind::X58),
        ]
        .into_iter()
        .filter_map(|(supported, kind)| supported.then_some(kind))
        .collect()),
        _ => bail!("未知独立灯条扩展能力页版本"),
    }
}
