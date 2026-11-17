//! OEM page2 布局与 cmd103 编码；模式值与 RGBKB.SetMode 一致。
use anyhow::{Result, anyhow, bail};
use ccc_core::settings::Patch;
use serde::{Deserialize, Serialize};

pub(super) const CMD_RGB: i32 = 103;
pub(super) const CMD_WHITE: i32 = 39;
pub(super) const RGB_BRIGHTNESS_RAW: [u32; 5] = [0, 47, 95, 143, 191];
pub(super) const PAGE: u8 = 2;
pub(super) const BLOCK_OFFSET: usize = 32;
pub(super) const BLOCK_LEN: usize = 76;
pub(super) const fn idx(offset: usize) -> usize {
    offset - BLOCK_OFFSET
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum KeyboardFamily {
    ZoneRgb,
    White,
    Perkey,
    MultiZone,
    None,
    #[default]
    Unknown,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Effect {
    Random,
    Zones,
    Breath,
    Cycle,
    Wave,
    Dance,
    Tempo,
    Flash,
    Solid,
}
impl Effect {
    pub(super) fn from_raw(raw: u8) -> Option<Self> {
        Some(match raw {
            0 => Self::Random,
            1 => Self::Zones,
            2 => Self::Breath,
            3 => Self::Cycle,
            4 => Self::Wave,
            5 => Self::Dance,
            6 => Self::Tempo,
            7 => Self::Flash,
            8 => Self::Solid,
            _ => return None,
        })
    }
    pub(super) fn raw(self) -> u8 {
        match self {
            Self::Random => 0,
            Self::Zones => 1,
            Self::Breath => 2,
            Self::Cycle => 3,
            Self::Wave => 4,
            Self::Dance => 5,
            Self::Tempo => 6,
            Self::Flash => 7,
            Self::Solid => 8,
        }
    }
    pub(super) fn word(self) -> Option<u32> {
        match self {
            Self::Random => Some(0x7000_0000),
            Self::Breath => Some(0x1002_A000),
            Self::Cycle => Some(0x3301_0000),
            Self::Wave => Some(0xB000_0000),
            Self::Dance => Some(0x8000_0000),
            Self::Tempo => Some(0x9000_0000),
            Self::Flash => Some(0xA000_0000),
            Self::Zones | Self::Solid => None,
        }
    }
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Zone {
    pub enabled: bool,
    pub color: Rgb,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "family", rename_all = "camelCase", deny_unknown_fields)]
pub enum LightingRequest {
    #[serde(rename_all = "camelCase")]
    ZoneRgb {
        brightness: u8,
        effect: Effect,
        all_color: Rgb,
        zones: [Zone; 3],
    },
    White {
        brightness: u8,
    },
}
impl LightingRequest {
    pub(super) fn enabled(&self) -> bool {
        match self {
            Self::White { brightness } | Self::ZoneRgb { brightness, .. } => *brightness > 0,
        }
    }
    pub(super) fn family(&self) -> KeyboardFamily {
        match self {
            Self::ZoneRgb { .. } => KeyboardFamily::ZoneRgb,
            Self::White { .. } => KeyboardFamily::White,
        }
    }
    pub(super) fn validate(&self) -> Result<()> {
        match self {
            Self::ZoneRgb { brightness, .. } if *brightness > 4 => bail!("RGB 亮度级别须为 0..=4"),
            Self::White { brightness } if *brightness > 5 => bail!("白光亮度级别须为 0..=5"),
            _ => Ok(()),
        }
    }
    pub(super) fn patches(&self) -> Vec<Patch> {
        match self {
            Self::White { brightness } => vec![
                Patch::new(35, [*brightness]),
                Patch::new(80, [u8::from(*brightness > 0)]),
            ],
            Self::ZoneRgb {
                brightness,
                effect,
                all_color,
                zones,
            } => {
                let mut patches = vec![
                    Patch::new(32, [effect.raw()]),
                    Patch::new(35, [*brightness]),
                    Patch::new(80, [u8::from(*brightness > 0)]),
                    Patch::new(81, [all_color.r, all_color.g, all_color.b]),
                ];
                for (at, zone) in [84usize, 88, 92].into_iter().zip(zones) {
                    patches.push(Patch::new(
                        at,
                        [
                            u8::from(zone.enabled),
                            zone.color.r,
                            zone.color.g,
                            zone.color.b,
                        ],
                    ));
                }
                patches
            }
        }
    }
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyboardSnapshot {
    pub brightness_level: u8,
    pub enabled: bool,
    pub raw_mode: u8,
    pub effect: Option<Effect>,
    pub all_color: Rgb,
    pub zones: [Zone; 3],
}
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LightingProfile {
    pub available: bool,
    pub unavailable_reason: Option<String>,
    pub detected_type: Option<u8>,
    pub detected_family: KeyboardFamily,
    pub detection_source: Option<String>,
    pub current: Option<KeyboardSnapshot>,
    pub baseline: Option<KeyboardSnapshot>,
    pub warnings: Vec<String>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyOutcome {
    pub ok: bool,
    pub message: String,
    pub rollback_error: Option<String>,
}
pub(super) fn failure(message: String) -> ApplyOutcome {
    ApplyOutcome {
        ok: false,
        message,
        rollback_error: None,
    }
}
pub(super) fn family_from_type(raw: u8) -> KeyboardFamily {
    match raw {
        1 => KeyboardFamily::White,
        2 | 6 | 22 | 38 => KeyboardFamily::ZoneRgb,
        3 | 19 | 35 | 51 | 243 => KeyboardFamily::Perkey,
        23 => KeyboardFamily::MultiZone,
        0 => KeyboardFamily::None,
        _ => KeyboardFamily::Unknown,
    }
}
/// LedKeyboardSetting 6.26 RGBKB.SetColor：6/22 不缩放蓝色，春绿色另有固件编码；普通型号按 .NET 偶数舍入。
pub(super) fn encode_zone_color(zone: u8, color: Rgb, raw_type: Option<u8>) -> u32 {
    let blue = if matches!(raw_type, Some(6 | 22)) {
        if color
            == (Rgb {
                r: 0,
                g: 255,
                b: 127,
            })
        {
            70
        } else {
            color.b
        }
    } else {
        ((40.0 / 51.0) * f64::from(color.b)).round_ties_even() as u8
    };
    (u32::from(zone) << 24)
        | (u32::from(blue) << 16)
        | (u32::from(color.r) << 8)
        | u32::from(color.g)
}
pub(super) fn encode_rgb_brightness_word(level: u8, raw_type: Option<u8>) -> Result<u32> {
    let levels = if raw_type == Some(38) {
        [47, 47, 102, 155, 212]
    } else {
        RGB_BRIGHTNESS_RAW
    };
    levels
        .get(usize::from(level))
        .map(|raw| 0xF400_0000 | raw)
        .ok_or_else(|| anyhow!("RGB 亮度级别须为 0..=4"))
}
pub(super) fn encode_onoff_mask(
    block: &SavedBlock,
    zones: &[Zone; 3],
    enable: bool,
    light_bar: bool,
) -> u32 {
    if !enable {
        return 0xE000_0007;
    }
    let tp_lit = block.bytes[idx(96)] != 0 && block.color(idx(97)) != (Rgb { r: 0, g: 0, b: 0 });
    let mut mask = 0xE000_1000 | if tp_lit { 1 } else { 7 };
    for (n, zone) in zones.iter().enumerate() {
        mask |= u32::from(zone.enabled) << (16 + n);
    }
    if block.bytes[idx(100)] != 0 {
        mask |= 1 << 13;
    }
    if block.bytes[idx(if light_bar { 104 } else { 96 })] != 0 {
        mask |= 1 << 15;
    }
    mask
}
pub(super) struct SavedBlock {
    pub(super) bytes: Vec<u8>,
}
impl SavedBlock {
    pub(super) fn from_bytes(bytes: Vec<u8>) -> Result<Self> {
        if bytes.len() != BLOCK_LEN {
            bail!("page2 键盘灯保存块长度 {} ≠ {}", bytes.len(), BLOCK_LEN);
        }
        Ok(Self { bytes })
    }
    pub(super) fn color(&self, at: usize) -> Rgb {
        Rgb {
            r: self.bytes[at],
            g: self.bytes[at + 1],
            b: self.bytes[at + 2],
        }
    }
    pub(super) fn snapshot(&self) -> KeyboardSnapshot {
        KeyboardSnapshot {
            brightness_level: self.bytes[idx(35)],
            enabled: self.bytes[idx(80)] != 0,
            raw_mode: self.bytes[idx(32)],
            effect: Effect::from_raw(self.bytes[idx(32)]),
            all_color: self.color(idx(81)),
            zones: [84, 88, 92].map(|at| Zone {
                enabled: self.bytes[idx(at)] != 0,
                color: self.color(idx(at + 1)),
            }),
        }
    }
    pub(super) fn request(&self, family: KeyboardFamily) -> Result<LightingRequest> {
        let snapshot = self.snapshot();
        let request = match family {
            KeyboardFamily::ZoneRgb => LightingRequest::ZoneRgb {
                brightness: snapshot.brightness_level,
                effect: snapshot.effect.ok_or_else(|| {
                    anyhow!("保存灯效 {} 未识别，无法保证恢复", snapshot.raw_mode)
                })?,
                all_color: snapshot.all_color,
                zones: snapshot.zones,
            },
            KeyboardFamily::White => LightingRequest::White {
                brightness: snapshot.brightness_level,
            },
            _ => bail!("不支持的键盘类型"),
        };
        request.validate()?;
        Ok(request)
    }
}
