use anyhow::{Result, anyhow, bail};
use ccc_core::{
    fan::{
        OemCurvePoints, OemFanAnchors, OemFanCurveInput, oem_page4_offsets, oem_slope,
        validate_oem_curve,
    },
    settings::Patch,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FanPointSet {
    pub id: String,
    pub present: bool,
    pub t1: f64,
    pub d1_pct: f64,
    pub t2: f64,
    pub d2_pct: f64,
    pub t3: f64,
    pub d3_pct: f64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FanProfile {
    #[serde(skip)]
    pub software_supported: bool,
    pub software: Option<ccc_core::software_fan::Curves>,
    pub curve_kind: CurveKind,
    pub rpm_fans: Vec<super::rpm::RpmFan>,
    pub available_modes: Vec<u8>,
    pub fan_count: u8,
    pub custom_supported: bool,
    #[serde(skip)]
    pub(crate) saved_mode: u8,
    pub current_mode: u8,
    pub offset_pct: u8,
    pub offset_slot: Option<usize>,
    pub offset_blocked_reason: Option<String>,
    pub can_restore: bool,
    pub fans: Vec<FanPointSet>,
    pub warnings: Vec<String>,
    pub safety_status: String,
    pub blocked_reason: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OffsetSource {
    pub mode: u8,
    #[serde(deserialize_with = "Option::deserialize")]
    pub slot: Option<usize>,
}

#[derive(Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CurveKind {
    TwoPoint,
    RpmTable,
}

pub fn decode(package: &[u8; 256], page: &[u8], capabilities: &[u8]) -> Result<FanProfile> {
    let count = package[12];
    let rpm_layout = super::rpm::supported(capabilities);
    if !(1..=if rpm_layout { 4 } else { 3 }).contains(&count)
        || page.len() != 256
        || capabilities.len() != 256
    {
        bail!("风扇能力数据布局未确认");
    }
    let fans = if rpm_layout {
        Vec::new()
    } else {
        decode_points(package, page)
    };
    let known_layout = capabilities[0..2] == [0, 0] || capabilities[0..2] == [1, 0];
    let supported = known_layout
        && package[43] & 2 == 0
        && if rpm_layout {
            package[50] & 15 != 0
        } else {
            count > 1
                && package[50] & 8 == 0
                && fans
                    .iter()
                    .filter(|f| f.present)
                    .all(|f| (0.0..98.0).contains(&f.t1) && (0.0..99.0).contains(&f.d1_pct))
        };
    Ok(FanProfile {
        software_supported: false,
        software: None,
        curve_kind: if rpm_layout {
            CurveKind::RpmTable
        } else {
            CurveKind::TwoPoint
        },
        rpm_fans: Vec::new(),
        available_modes: available_modes(package, capabilities, supported),
        fan_count: count,
        custom_supported: supported,
        saved_mode: page[5],
        current_mode: page[5],
        offset_pct: page[7],
        offset_slot: None,
        offset_blocked_reason: None,
        can_restore: false,
        fans,
        warnings: if supported {
            vec![]
        } else {
            vec![
                if rpm_layout {
                    "固件未开放十级 RPM 自定义曲线"
                } else {
                    "曲线布局待校准"
                }
                .into(),
            ]
        },
        safety_status: "固件控制".into(),
        blocked_reason: None,
    })
}

fn decode_points(package: &[u8; 256], page: &[u8]) -> Vec<FanPointSet> {
    let mut fans = Vec::new();
    for (index, id) in ["cpu", "gpu1", "gpu2"].into_iter().enumerate() {
        let at = 16 + index * 8;
        let (d, t, _) = oem_page4_offsets(index);
        let saved = page[d + 1] != 0;
        fans.push(FanPointSet {
            id: id.into(),
            present: if package[50] == 0 {
                index < package[12] as usize
            } else {
                package[50] & (1 << index) != 0
            },
            t1: package[at] as f64,
            d1_pct: (package[at + 1] as f64 / 255.0 * 100.0).round_ties_even(),
            t2: if saved {
                page[t + 1] as f64
            } else {
                package[at + 2] as f64
            },
            d2_pct: if saved {
                page[d + 1] as f64
            } else {
                (package[at + 3] as f64 / 255.0 * 100.0).round_ties_even()
            },
            t3: if saved {
                page[t + 2] as f64
            } else {
                package[at + 4] as f64
            },
            d3_pct: if saved {
                page[d + 2] as f64
            } else {
                (package[at + 5] as f64 / 255.0 * 100.0).round_ties_even()
            },
        });
    }
    fans
}

fn available_modes(package: &[u8; 256], capabilities: &[u8], custom: bool) -> Vec<u8> {
    let mut modes = vec![super::FAN_MODE_AUTO, super::FAN_MODE_FULL_SPEED];
    if package[14] == 5 {
        modes.push(5);
    }
    if custom {
        modes.push(6);
    }
    if supports_noiseless(capabilities) {
        modes.push(super::FAN_MODE_NOISELESS);
    }
    modes
}

pub(crate) fn supports_noiseless(capabilities: &[u8]) -> bool {
    capabilities.len() > 20 && capabilities[..2] == [1, 0] && capabilities[20] & 1 != 0
}

pub fn prepare(
    profile: &FanProfile,
    wanted: [Option<OemCurvePoints>; 3],
) -> Result<[Option<OemFanCurveInput>; 3]> {
    if !profile.custom_supported || profile.curve_kind != CurveKind::TwoPoint {
        bail!("固件曲线能力或布局未确认");
    }
    let mut inputs = [None; 3];
    for (index, set) in profile.fans.iter().enumerate() {
        if !set.present {
            if wanted[index].is_some() {
                bail!("{} 风扇不存在", set.id);
            }
            continue;
        }
        let mut points = wanted[index].unwrap_or(OemCurvePoints {
            t2: set.t2,
            d2: set.d2_pct,
            t3: set.t3,
            d3: set.d3_pct,
        });
        points.t2 = points.t2.round();
        points.t3 = points.t3.round();
        points.d2 = points.d2.round();
        points.d3 = points.d3.round();
        let input = OemFanCurveInput {
            anchors: OemFanAnchors {
                t1: set.t1,
                d1: set.d1_pct,
            },
            points,
        };
        validate_oem_curve(&input).map_err(|e| anyhow!("{}：{e}", set.id))?;
        inputs[index] = Some(input);
    }
    Ok(inputs)
}

pub fn page_patches(inputs: &[Option<OemFanCurveInput>; 3]) -> Vec<Patch> {
    let mut patches = Vec::new();
    for (index, input) in inputs.iter().enumerate() {
        let Some(input) = input else { continue };
        let (d, t, s) = oem_page4_offsets(index);
        let p = &input.points;
        patches.push(Patch::new(
            d,
            [input.anchors.d1.round() as u8, p.d2 as u8, p.d3 as u8, 100],
        ));
        patches.push(Patch::new(
            t,
            [input.anchors.t1 as u8, p.t2 as u8, p.t3 as u8, 100],
        ));
        let mut slopes = Vec::new();
        for slope in [
            oem_slope(input.anchors.d1, p.d2, input.anchors.t1, p.t2),
            oem_slope(p.d2, p.d3, p.t2, p.t3),
            oem_slope(p.d3, 100.0, p.t3, 100.0),
        ] {
            slopes.extend_from_slice(&(slope as u16).to_le_bytes());
        }
        patches.push(Patch::new(s, slopes));
    }
    patches
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recorded_two_fan_packet_does_not_hide_all_fans_on_zero_bitmap() {
        let mut packet = [0u8; 256];
        packet[12] = 2;
        packet[16..32].copy_from_slice(&[
            51, 58, 60, 82, 80, 84, 100, 255, 55, 71, 60, 74, 80, 94, 99, 255,
        ]);
        let profile = decode(&packet, &[0; 256], &[0; 256]).unwrap();
        assert_eq!(profile.fans.iter().filter(|f| f.present).count(), 2);
        let inputs = prepare(&profile, [None, None, None]).unwrap();
        assert!(inputs[2].is_none());
        assert!(!page_patches(&inputs).iter().any(|p| p.offset >= 52));
    }

    #[test]
    fn page4_slopes_use_little_endian_unlike_cmd14() {
        let input = OemFanCurveInput {
            anchors: OemFanAnchors { t1: 40.0, d1: 20.0 },
            points: OemCurvePoints {
                t2: 55.0,
                d2: 40.0,
                t3: 70.0,
                d3: 60.0,
            },
        };
        let patches = page_patches(&[Some(input), None, None]);
        assert_eq!(
            patches.iter().find(|p| p.offset == 28).unwrap().data,
            [54, 0, 54, 0, 54, 0]
        );
    }

    #[test]
    fn only_declared_modes_and_curve_layout_are_exposed() {
        let mut packet = [0; 256];
        packet[12] = 2;
        let mut capabilities = [0; 256];
        capabilities[0] = 1;
        capabilities[20] = 0x11;
        let profile = decode(&packet, &[0; 256], &capabilities).unwrap();
        assert_eq!(profile.available_modes, [0, 1, 8]);
        assert!(!profile.custom_supported);
        assert!(prepare(&profile, [None; 3]).is_err());
    }
}
