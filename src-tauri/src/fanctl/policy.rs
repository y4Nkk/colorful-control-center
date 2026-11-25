use super::{FAN_MODE_AUTO, FAN_MODE_MAXQ, FAN_MODE_NOISELESS, validate_mode};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(
    tag = "control",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub(crate) enum ModePolicy {
    Firmware {
        saved_mode: u8,
    },
    Explicit {
        mode: u8,
        saved_mode: u8,
        #[serde(deserialize_with = "Option::deserialize")]
        offset_pct: Option<u8>,
    },
}

impl ModePolicy {
    pub const fn explicit(mode: u8, saved_mode: u8, offset_pct: Option<u8>) -> Self {
        Self::Explicit {
            mode,
            saved_mode,
            offset_pct,
        }
    }

    pub fn mode(self) -> Option<u8> {
        match self {
            Self::Firmware { .. } => None,
            Self::Explicit { mode, .. } => Some(mode),
        }
    }

    pub fn offset_pct(self) -> Option<u8> {
        match self {
            Self::Firmware { .. } => None,
            Self::Explicit { offset_pct, .. } => offset_pct,
        }
    }

    pub(super) fn saved_mode(self) -> u8 {
        match self {
            Self::Firmware { saved_mode } | Self::Explicit { saved_mode, .. } => saved_mode,
        }
    }

    pub fn validate(self) -> Result<()> {
        let saved = self.saved_mode();
        match self {
            Self::Firmware { .. } => ensure!(
                matches!(saved, FAN_MODE_AUTO | FAN_MODE_MAXQ),
                "固件风扇策略的基础档位无效"
            ),
            Self::Explicit {
                mode, offset_pct, ..
            } => {
                validate_mode(mode)?;
                ensure!(
                    saved == mode
                        || mode == FAN_MODE_NOISELESS
                            && matches!(saved, FAN_MODE_AUTO | FAN_MODE_MAXQ),
                    "风扇保存策略与即时模式不一致"
                );
                ensure!(
                    offset_pct.is_none_or(|pct| pct <= 100),
                    "原厂分模式风扇偏移超出范围"
                );
            }
        }
        Ok(())
    }

    pub fn validate_turbo(self, turbo: bool) -> Result<()> {
        ensure!(
            self.mode().is_some() || !turbo,
            "Turbo 风扇缺少明确的档位目标"
        );
        if let Some(mode) = self.mode() {
            super::boost::validate_active_fan(turbo, super::FanTarget::Firmware(mode))?;
        }
        Ok(())
    }
}
