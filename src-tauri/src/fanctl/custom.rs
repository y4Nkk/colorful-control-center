//! 本地自定义配置独立于当前档位。调用方持 HAL 写锁协调保存与显式切档。
use super::{CurveKind, CurvePoints, FAN_MODE_CUSTOM, FanProfile, RestoreView, profile, rpm};
use crate::hal::Hal;
use anyhow::{Context, Result, ensure};
use ccc_core::fan::OemCurvePoints;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, io::ErrorKind, path::PathBuf};

pub(crate) const RECORD_NAME: &str = "custom-fan-curves.json";
const TWO_POINT_FANS: [&str; 3] = ["cpu", "gpu1", "gpu2"];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub(crate) enum Curves {
    TwoPoint {
        curves: BTreeMap<String, CurvePoints>,
    },
    RpmTable {
        curves: rpm::Curves,
    },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Configuration {
    pub(crate) revision: String,
    pub(crate) saved: Option<Curves>,
    pub(crate) pending: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct View {
    #[serde(flatten)]
    firmware: RestoreView,
    pub(crate) custom: Option<Configuration>,
    custom_error: Option<String>,
}

pub(crate) struct Store(pub(crate) PathBuf);

impl Store {
    fn load(&self) -> Result<Option<Curves>> {
        match fs::read(&self.0) {
            Ok(bytes) => Ok(Some(
                serde_json::from_slice(&bytes).context("自定义风扇配置格式无效")?,
            )),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error).context("无法读取本地自定义风扇配置"),
        }
    }

    fn configuration(&self, profile: &FanProfile) -> Result<Configuration> {
        let saved = self.load()?;
        let pending = saved
            .as_ref()
            .is_some_and(|value| value.differs_from(profile));
        let source =
            serde_json::to_vec(&(profile.curve_kind, &profile.fans, &profile.rpm_fans, &saved))?;
        Ok(Configuration {
            revision: format!("{:x}", Sha256::digest(source)),
            saved,
            pending,
        })
    }

    pub(crate) fn read(&self, hal: &Hal) -> Result<View> {
        let firmware = super::read_restore_view(hal)?;
        let (custom, custom_error) = if firmware.profile.custom_supported {
            match self.configuration(&firmware.profile) {
                Ok(configuration) => (Some(configuration), None),
                Err(error) => (None, Some(format!("{error:#}"))),
            }
        } else {
            (None, None)
        };
        Ok(View {
            firmware,
            custom,
            custom_error,
        })
    }

    pub(crate) fn save(&self, hal: &Hal, expected: &str, changes: Curves) -> Result<()> {
        let profile = super::read_profile(hal)?;
        ensure!(profile.custom_supported, "固件未声明支持自定义风扇表");
        ensure!(!changes.is_empty(), "没有修改的风扇曲线");
        let current = self.configuration(&profile)?;
        ensure!(
            current.revision == expected,
            "风扇曲线配置已变化，请重新打开编辑"
        );
        let mut next = changes.merge(current.saved);
        next.validate(hal, &profile)?;
        let bytes = serde_json::to_vec_pretty(&next)?;
        fs::create_dir_all(self.0.parent().context("自定义风扇配置目录不可用")?)
            .context("无法创建自定义风扇配置目录")?;
        crate::storage::write(&self.0, &bytes).context("无法保存本地自定义风扇配置")
    }

    pub(crate) fn select_mode(&self, hal: &Hal, mode: u8) -> Result<u8> {
        if mode != FAN_MODE_CUSTOM {
            return super::set_mode(hal, mode);
        }
        let Some(mut saved) = self.load()? else {
            return super::set_mode(hal, mode);
        };
        let profile = super::read_profile(hal)?;
        saved.validate(hal, &profile)?;
        let active = profile.software.is_none() && profile.current_mode == mode;
        if !active || saved.differs_from(&profile) {
            match saved {
                Curves::TwoPoint { curves } => {
                    let [cpu, gpu1, gpu2] = points(&curves);
                    if active {
                        super::edit_curve(hal, [cpu, gpu1, gpu2])?;
                    } else {
                        super::apply_curve(hal, cpu, gpu1, gpu2)?;
                    }
                }
                Curves::RpmTable { curves } => {
                    if active {
                        rpm::edit(hal, &curves)?;
                    } else {
                        rpm::apply(hal, &curves)?;
                    }
                }
            }
        }
        Ok(mode)
    }
}

fn points(values: &BTreeMap<String, CurvePoints>) -> [Option<OemCurvePoints>; 3] {
    TWO_POINT_FANS.map(|id| values.get(id).copied().map(Into::into))
}

impl Curves {
    fn is_empty(&self) -> bool {
        match self {
            Self::TwoPoint { curves } => curves.is_empty(),
            Self::RpmTable { curves } => curves.is_empty(),
        }
    }

    fn merge(self, previous: Option<Self>) -> Self {
        match (self, previous) {
            (Self::TwoPoint { curves }, Some(Self::TwoPoint { curves: mut saved })) => {
                saved.extend(curves);
                Self::TwoPoint { curves: saved }
            }
            (Self::RpmTable { curves }, Some(Self::RpmTable { curves: mut saved })) => {
                saved.extend(curves);
                Self::RpmTable { curves: saved }
            }
            (changes, _) => changes,
        }
    }

    fn validate(&mut self, hal: &Hal, firmware: &FanProfile) -> Result<()> {
        ensure!(firmware.custom_supported, "固件未声明支持自定义风扇表");
        ensure!(!self.is_empty(), "自定义风扇配置没有曲线");
        match (self, firmware.curve_kind) {
            (Self::TwoPoint { curves }, CurveKind::TwoPoint) => {
                ensure!(
                    curves
                        .keys()
                        .all(|id| TWO_POINT_FANS.contains(&id.as_str())),
                    "自定义配置包含未知风扇"
                );
                let prepared = profile::prepare(firmware, points(curves))?;
                for (id, input) in TWO_POINT_FANS.into_iter().zip(prepared) {
                    if let Some(input) = input {
                        curves.insert(id.into(), input.points.into());
                    }
                }
                Ok(())
            }
            (Self::RpmTable { curves }, CurveKind::RpmTable) => rpm::validate(hal, curves),
            _ => anyhow::bail!("自定义配置与当前固件曲线类型不符，请重新编辑"),
        }
    }

    fn differs_from(&self, firmware: &FanProfile) -> bool {
        match (self, firmware.curve_kind) {
            (Self::TwoPoint { curves }, CurveKind::TwoPoint) => {
                curves.iter().any(|(id, points)| {
                    !firmware.fans.iter().any(|fan| {
                        fan.present && &fan.id == id && CurvePoints::from(fan) == *points
                    })
                })
            }
            (Self::RpmTable { curves }, CurveKind::RpmTable) => {
                curves.iter().any(|(id, points)| {
                    !firmware
                        .rpm_fans
                        .iter()
                        .any(|fan| &fan.id == id && fan.points == *points)
                })
            }
            _ => true,
        }
    }
}
