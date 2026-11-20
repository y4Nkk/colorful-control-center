//! 档案数据与校验的唯一契约；运行时执行与磁盘入口复用。
#[path = "groups.rs"]
mod groups;
pub use groups::ProfileGroup;
use crate::{fanctl, powerctl};
use anyhow::{Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProfileFanPoints {
    pub t2: f64,
    pub d2_pct: f64,
    pub t3: f64,
    pub d3_pct: f64,
}

#[derive(Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct ProfileSettings {
    pub oem_power_mode: Option<u8>,
    pub performance_options: Option<powerctl::mode::boost::Values>,
    pub fan_mode: Option<u8>,
    pub fan_software_curves: Option<ccc_core::software_fan::Curves>,
    pub fan_rpm_curves: Option<fanctl::rpm::Curves>,
    pub fan_curves: Option<BTreeMap<String, ProfileFanPoints>>,
    pub runtime_cpu: Option<powerctl::runtime::Settings>,
    pub pl1_w: Option<f64>,
    pub pl2_w: Option<f64>,
    pub core_offset_mv: Option<i32>,
    pub gt_offset_mv: Option<i32>,
    pub gpu: Option<crate::gpuctl::GpuClockSettings>,
    pub light_bar: Option<crate::lighting::lightbar::Values>,
    pub dust_schedule: Option<fanctl::dust::Schedule>,
    pub keyboard: Option<crate::keyboard::Values>,
    pub microphones: Option<crate::microphone::Target>,
    pub touchpad: Option<crate::touchpad::Target>,
    pub keyboard_lighting: Option<crate::lighting::Target>,
    pub multi_zone_lighting: Option<crate::lighting::multizone::Target>,
    pub lighting_options: Option<crate::lighting::options::Target>,
    pub perkey_lighting: Option<crate::lighting::perkey::Target>,
    pub keymap: Option<crate::keymap::Target>,
    pub power_delivery: Option<crate::platform::Target>,
    pub usb_policy: Option<crate::platform::usb::Target>,
    pub battery_charge: Option<crate::battery::ChargeTarget>,
    pub battery_energy: Option<crate::battery::energy::Target>,
    pub lan_policy: Option<crate::platform::lan::Target>,
    pub display: Option<crate::display::profile::Target>,
    pub gpu_power: Option<crate::gpu_power::profile::Target>,
    pub graphics_mode: Option<crate::graphics_mode::profile::Target>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProfileFile {
    pub version: u32,
    pub name: String,
    pub saved_at_ms: u64,
    pub settings: ProfileSettings,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileMeta {
    pub name: String,
    pub groups: Vec<ProfileGroup>,
    pub revision: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyStep {
    pub step: String,
    pub ok: bool,
    pub message: String,
}

pub fn validate(settings: &ProfileSettings) -> Result<()> {
    validate_devices(settings)?;
    if let Some(schedule) = settings.dust_schedule {
        schedule.validate()?;
        if schedule.enabled() && settings.fan_software_curves.is_some() {
            bail!("软件风扇曲线不能与启用的除尘计划同时配置");
        }
    }
    if let Some(target) = &settings.light_bar {
        target.validate()?;
    }
    if let Some(target) = &settings.runtime_cpu {
        powerctl::runtime::validate(target)?;
    }
    if settings.oem_power_mode.is_some_and(|mode| mode > 4) {
        bail!("OEM 电源模式须在 0–4 范围");
    }
    if let Some(mode) = settings.fan_mode {
        fanctl::validate_mode(mode)?;
    }
    if settings.pl1_w.is_some() || settings.pl2_w.is_some() {
        powerctl::validate_pl(settings.pl1_w, settings.pl2_w)?;
    }
    if settings.core_offset_mv.is_some() || settings.gt_offset_mv.is_some() {
        powerctl::validate_voltage(settings.core_offset_mv, settings.gt_offset_mv)?;
    }
    validate_curves(settings)?;
    if let Some(gpu) = settings.gpu {
        if gpu
            .core_offset_mhz
            .is_some_and(|v| !(-300..=300).contains(&v))
            || gpu
                .mem_offset_mhz
                .is_some_and(|v| !(-1000..=1000).contains(&v))
        {
            bail!("GPU 偏移超出允许范围");
        }
        if gpu.core_offset_mhz.is_none() && gpu.mem_offset_mhz.is_none() {
            bail!("GPU 设置为空");
        }
    }
    if !ProfileGroup::ALL
        .into_iter()
        .any(|group| group.includes(settings))
    {
        bail!("档案没有设置");
    }
    Ok(())
}

fn validate_devices(settings: &ProfileSettings) -> Result<()> {
    if let Some(target) = &settings.touchpad {
        target.validate()?;
    }
    validate_multi_zone(settings)?;
    if let Some(target) = &settings.microphones {
        target.validate()?;
    }
    if let Some(target) = &settings.graphics_mode {
        target.validate()?;
    }

    if let Some(target) = &settings.gpu_power {
        target.validate()?;
        if !target.values.enabled
            && (settings.gpu.is_some() || settings.fan_software_curves.is_some())
        {
            bail!("独显关闭目标不能同时配置GPU偏移或软件风扇曲线");
        }
    }
    if let Some(target) = &settings.display {
        target.validate()?;
    }
    if let Some(target) = &settings.lan_policy {
        target.validate()?;
    }
    if let Some(values) = &settings.performance_options {
        values.validate()?;
        if let (Some(mode), Some(fan)) = (settings.oem_power_mode, fan_target(settings)) {
            powerctl::mode::boost::validate_fan(mode, values.turbo_fan == Some(true), fan)?;
        }
    }
    crate::battery::validate_targets(
        settings.battery_charge.as_ref(),
        settings.battery_energy.as_ref(),
    )?;
    if let Some(target) = &settings.power_delivery {
        target.validate()?;
    }
    if let Some(target) = &settings.usb_policy {
        target.validate()?;
    }
    if let Some(target) = &settings.keymap {
        target.validate()?;
    }
    if let Some(target) = &settings.perkey_lighting {
        target.validate()?;
        if settings.keyboard_lighting.is_some() {
            bail!("档案不能同时包含普通键盘灯与逐键灯效");
        }
        if settings
            .lighting_options
            .as_ref()
            .is_some_and(|options| options.keyboard_type != target.keyboard_type)
        {
            bail!("逐键灯效与灯光策略的键盘型号不一致");
        }
    }
    if settings
        .keyboard
        .as_ref()
        .is_some_and(|values| values.is_empty())
    {
        bail!("档案中的键盘设置为空");
    }
    if let Some(target) = &settings.keyboard_lighting {
        target.validate()?;
    }
    if let Some(target) = &settings.lighting_options {
        target.validate()?;
    }
    if let (Some(lighting), Some(options)) =
        (&settings.keyboard_lighting, &settings.lighting_options)
    {
        if lighting.keyboard_type != options.keyboard_type {
            bail!("档案的键盘灯与灯光策略型号不一致");
        }
    }
    Ok(())
}

pub(super) fn fan_target(settings: &ProfileSettings) -> Option<powerctl::mode::boost::FanTarget> {
    use powerctl::mode::boost::FanTarget;
    if settings.fan_software_curves.is_some() {
        Some(FanTarget::Software)
    } else if let Some(mode) = settings.fan_mode {
        Some(FanTarget::Firmware(mode))
    } else if settings.fan_curves.is_some() || settings.fan_rpm_curves.is_some() {
        Some(FanTarget::Firmware(6))
    } else {
        None
    }
}

fn validate_curves(settings: &ProfileSettings) -> Result<()> {
    if let Some(curves) = &settings.fan_software_curves {
        if settings.fan_mode.is_some()
            || settings.fan_curves.is_some()
            || settings.fan_rpm_curves.is_some()
        {
            bail!("软件曲线不能与固件风扇模式或曲线同时配置");
        }
        ccc_core::software_fan::validate(curves).map_err(|e| anyhow!(e))?;
    }
    if settings.fan_curves.is_some() && settings.fan_rpm_curves.is_some() {
        bail!("档案不能同时包含两点曲线和十级 RPM 表");
    }
    if let Some(curves) = &settings.fan_curves {
        if curves.is_empty() {
            bail!("风扇曲线为空");
        }
        for (id, curve) in curves {
            if !["cpu", "gpu1", "gpu2"].contains(&id.as_str()) {
                bail!("未知风扇 {id}");
            }
            if ![curve.t2, curve.t3, curve.d2_pct, curve.d3_pct]
                .iter()
                .all(|n| n.is_finite())
                || !(0.0 < curve.t2 && curve.t2 < curve.t3 && curve.t3 < 100.0)
                || !(20.0 <= curve.d2_pct && curve.d2_pct < curve.d3_pct && curve.d3_pct < 100.0)
            {
                bail!("{id} 曲线无效");
            }
        }
    }
    if let Some(curves) = &settings.fan_rpm_curves {
        if curves.is_empty() {
            bail!("RPM 曲线为空");
        }
        for (id, points) in curves {
            if !ccc_core::rpm::FAN_IDS.contains(&id.as_str()) {
                bail!("未知 RPM 风扇 {id}");
            }
            ccc_core::rpm::validate_curve(points).map_err(|e| anyhow::anyhow!("{id}：{e}"))?;
        }
    }
    Ok(())
}

pub fn capture_fan(fan: &fanctl::FanProfile, settings: &mut ProfileSettings) -> Result<()> {
    if let Some(curves) = &fan.software {
        settings.fan_mode = None;
        settings.fan_curves = None;
        settings.fan_rpm_curves = None;
        settings.fan_software_curves = Some(curves.clone());
        return Ok(());
    }
    settings.fan_software_curves = None;
    settings.fan_mode = Some(fan.current_mode);
    if !fan.custom_supported {
        return Ok(());
    }
    if fan.curve_kind == fanctl::CurveKind::TwoPoint {
        settings.fan_curves = Some(
            fan.fans
                .iter()
                .filter(|f| f.present)
                .map(|f| {
                    (
                        f.id.clone(),
                        ProfileFanPoints {
                            t2: f.t2,
                            d2_pct: f.d2_pct,
                            t3: f.t3,
                            d3_pct: f.d3_pct,
                        },
                    )
                })
                .collect(),
        );
    } else {
        let curves: fanctl::rpm::Curves = fan
            .rpm_fans
            .iter()
            .map(|f| (f.id.clone(), f.points))
            .collect();
        let validity = curves.values().try_for_each(ccc_core::rpm::validate_curve);
        match validity {
            Ok(()) => settings.fan_rpm_curves = Some(curves),
            Err(error) if fan.current_mode == 6 => {
                return Err(anyhow!("当前自定义 RPM 表不符合新目标限制：{error}"));
            }
            // 自动/预设模式只捕获生效的策略；不把原厂停转表伪造成 20% 手动目标。
            Err(_) => (),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn software_profiles_are_exclusive_and_capture_the_live_configuration() {
        let mut fan = fanctl::FanProfile {
            software_supported: true,
            software: None,
            curve_kind: fanctl::CurveKind::TwoPoint,
            rpm_fans: vec![],
            available_modes: vec![0, 1, 6],
            fan_count: 2,
            custom_supported: true,
            current_mode: 0,
            offset_pct: 0,
            can_restore: false,
            fans: vec![],
            warnings: vec![],
            safety_status: "自动".into(),
            blocked_reason: None,
        };
        let curve = ccc_core::fan::FanCurve::new(
            vec![
                ccc_core::fan::CurvePoint {
                    temp_c: 40.0,
                    duty_pct: 20.0,
                },
                ccc_core::fan::CurvePoint {
                    temp_c: 95.0,
                    duty_pct: 100.0,
                },
            ],
            3.0,
            10.0,
        );
        fan.software = Some(BTreeMap::from([("cpu".into(), curve)]));
        let mut s = ProfileSettings::default();
        capture_fan(&fan, &mut s).unwrap();
        assert_eq!(s.fan_software_curves, fan.software);
        assert!(s.fan_mode.is_none() && s.fan_curves.is_none() && s.fan_rpm_curves.is_none());
        assert!(validate(&s).is_ok());
        let json = serde_json::to_string(&s).unwrap();
        let decoded: ProfileSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.fan_software_curves, s.fan_software_curves);
        s.fan_mode = Some(0);
        assert!(validate(&s).is_err());
        s.fan_mode = None;
        s.fan_curves = Some(BTreeMap::new());
        assert!(validate(&s).is_err());
    }
    fn points() -> [ccc_core::rpm::RpmPoint; 10] {
        std::array::from_fn(|i| ccc_core::rpm::RpmPoint {
            temp_c: 30 + i as u8 * 7,
            rpm_pct: if i == 9 { 100 } else { 20 + i as u8 * 8 },
        })
    }
    #[test]
    fn rpm_profiles_accept_the_same_manual_contract_as_runtime() {
        let mut s = ProfileSettings {
            fan_rpm_curves: Some(BTreeMap::from([("sys".into(), points())])),
            ..Default::default()
        };
        assert!(validate(&s).is_ok());
        s.fan_rpm_curves.as_mut().unwrap().get_mut("sys").unwrap()[0].rpm_pct = 0;
        assert!(validate(&s).is_err());
        s.fan_rpm_curves = Some(BTreeMap::from([("cpu".into(), points())]));
        s.fan_curves = Some(BTreeMap::new());
        assert!(validate(&s).is_err());
        s.fan_curves = None;
        s.fan_rpm_curves = Some(BTreeMap::from([("unknown".into(), points())]));
        assert!(validate(&s).is_err());
        s.fan_rpm_curves = Some(BTreeMap::new());
        assert!(validate(&s).is_err());
    }
    #[test]
    fn malformed_rpm_json_and_unknown_fields_are_rejected() {
        for json in [r#"{"fanRpmCurves":{"cpu":[]}}"#, r#"{"obsolete":true}"#] {
            assert!(serde_json::from_str::<ProfileSettings>(json).is_err());
        }
        assert!(validate(&ProfileSettings::default()).is_err());
    }
    #[test]
    fn capture_does_not_turn_factory_stop_values_into_manual_targets() {
        let mut f = fanctl::FanProfile {
            software_supported: false,
            software: None,
            curve_kind: fanctl::CurveKind::RpmTable,
            custom_supported: true,
            rpm_fans: vec![fanctl::rpm::RpmFan {
                id: "cpu".into(),
                maximum_rpm: 5000,
                points: points(),
            }],
            available_modes: vec![0, 1, 6],
            fan_count: 1,
            current_mode: 0,
            offset_pct: 0,
            can_restore: false,
            fans: vec![],
            warnings: vec![],
            safety_status: "自动".into(),
            blocked_reason: None,
        };
        f.rpm_fans[0].points[0].rpm_pct = 0;
        let mut s = ProfileSettings::default();
        capture_fan(&f, &mut s).unwrap();
        assert_eq!(s.fan_mode, Some(0));
        assert!(s.fan_rpm_curves.is_none());
        assert!(validate(&s).is_ok());
        f.current_mode = 6;
        assert!(capture_fan(&f, &mut ProfileSettings::default()).is_err());
        f.rpm_fans[0].points[0].rpm_pct = 20;
        let mut s = ProfileSettings::default();
        capture_fan(&f, &mut s).unwrap();
        assert_eq!(
            s.fan_rpm_curves.as_ref().unwrap()["cpu"],
            f.rpm_fans[0].points
        );
        assert!(validate(&s).is_ok());
    }
}
fn validate_multi_zone(settings: &ProfileSettings) -> Result<()> {
    if let Some(target) = &settings.multi_zone_lighting {
        target.validate()?;
        if settings.keyboard_lighting.is_some() || settings.perkey_lighting.is_some() {
            bail!("多区键盘不能与普通或逐键键盘灯目标同时配置");
        }
        if settings
            .lighting_options
            .as_ref()
            .is_some_and(|options| options.keyboard_type != 23)
        {
            bail!("多区键盘与灯光策略的键盘型号不一致");
        }
    }
    Ok(())
}
