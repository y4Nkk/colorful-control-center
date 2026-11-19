use super::{ProfileGroup, ProfileSettings, validate};
use crate::{fanctl, hal::Hal, powerctl};
use anyhow::{Result, anyhow, bail};
use ccc_core::msr;

pub(crate) fn capture_current(
    hal: &Hal,
    gpu: &crate::gpuctl::GpuController,
    devices: super::Devices<'_>,
    groups: &[ProfileGroup],
) -> Result<ProfileSettings> {
    if groups.is_empty() {
        bail!("至少选择一组配置");
    }
    let capture = collect(hal, gpu, devices, groups)?;
    if !capture.issues.is_empty() {
        bail!("{}", capture.issues.join("；"));
    }
    validate(&capture.settings)?;
    Ok(capture.settings)
}

pub(crate) fn compare_current(
    hal: &Hal,
    gpu: &crate::gpuctl::GpuController,
    devices: super::Devices<'_>,
    settings: &ProfileSettings,
) -> Result<ProfileSettings> {
    validate(settings)?;
    // The display, graphics mode and GPU node reviews already carry their own before values.
    let groups: Vec<_> = ProfileGroup::ALL
        .into_iter()
        .filter(|group| *group != ProfileGroup::Display && group.includes(settings))
        .collect();
    if groups.is_empty() {
        return Ok(ProfileSettings::default());
    }
    Ok(collect(hal, gpu, devices, &groups)?.settings)
}

#[derive(Default)]
struct Capture {
    settings: ProfileSettings,
    issues: Vec<String>,
}

fn collect(
    hal: &Hal,
    gpu: &crate::gpuctl::GpuController,
    devices: super::Devices<'_>,
    groups: &[ProfileGroup],
) -> Result<Capture> {
    use ProfileGroup::*;
    let has = |group| groups.contains(&group);
    // Match Devices::scopes lock order among the selected domains.
    let dust = acquire(has(Devices), || devices.dust.scope())?;
    let light_bar = acquire(has(Lighting), || devices.light_bar.scope())?;
    let multi_zone = acquire(has(Lighting), || devices.multi_zone.scope())?;
    let keyboard = acquire(has(Devices), || devices.keyboard.scope())?;
    let microphones = acquire(has(Devices), || devices.microphones.scope())?;
    let touchpad = acquire(has(Devices), || devices.touchpad.scope())?;
    let lighting = acquire(has(Lighting), || devices.lighting.scope())?;
    let lighting_options = acquire(has(Lighting), || devices.lighting_options.scope())?;
    let mut perkey = acquire(has(Lighting), || devices.perkey.scope())?;
    let keymap = acquire(has(Devices), || devices.keymap.scope())?;
    let pd = acquire(has(Power), || devices.power_delivery.scope())?;
    let usb = acquire(has(Power), || devices.usb.scope())?;
    let battery = acquire(has(Power), || devices.battery.scope())?;
    let lan = acquire(has(Power), || devices.lan.scope())?;
    let node = acquire(has(Performance) || has(Display), || {
        devices.gpu_power.scope()
    })?;
    let graphics = acquire(has(Display), || devices.graphics_mode.scope())?;
    let _write = hal.begin_write()?;
    let Capture {
        mut settings,
        mut issues,
    } = Capture::default();
    macro_rules! read_targets {
        ($($field:ident, $label:literal, $scope:expr);+ $(;)?) => {
            $(settings.$field = observe(&mut issues, $label, $scope.map(|scope| scope.capture(hal))).flatten();)+
        };
    }
    read_targets! {
        dust_schedule, "除尘计划", dust.as_ref();
        light_bar, "灯条", light_bar.as_ref();
        multi_zone_lighting, "多区灯光", multi_zone.as_ref();
        keyboard, "键盘功能", keyboard.as_ref();
        microphones, "麦克风", microphones.as_ref();
        touchpad, "触摸板", touchpad.as_ref();
        keyboard_lighting, "键盘灯光", lighting.as_ref();
        lighting_options, "灯光策略", lighting_options.as_ref();
        perkey_lighting, "逐键灯光", perkey.as_mut();
        keymap, "键位与宏", keymap.as_ref();
        power_delivery, "PD 供电", pd.as_ref();
        usb_policy, "USB 供电", usb.as_ref();
        lan_policy, "有线网卡", lan.as_ref();
        graphics_mode, "显卡模式", graphics.as_ref();
    }
    if let Some((charge, energy)) = observe(
        &mut issues,
        "电池",
        battery.as_ref().map(|scope| scope.capture(hal)),
    ) {
        settings.battery_charge = charge;
        settings.battery_energy = energy;
    }
    if has(Display) {
        settings.display = observe(&mut issues, "显示", Some(devices.display.capture())).flatten();
        settings.gpu_power = observe(
            &mut issues,
            "独显电源",
            node.as_ref().map(|scope| scope.capture(hal)),
        )
        .flatten();
    }
    if let Some(node) = node.as_ref().filter(|_| has(Performance)) {
        capture_performance(hal, gpu, node, &mut settings, &mut issues);
    }
    Ok(Capture { settings, issues })
}

fn acquire<T>(selected: bool, scope: impl FnOnce() -> Result<T>) -> Result<Option<T>> {
    selected.then(scope).transpose()
}

fn observe<T>(issues: &mut Vec<String>, label: &str, value: Option<Result<T>>) -> Option<T> {
    value?
        .map_err(|error| issues.push(format!("{label}：{error:#}")))
        .ok()
}

fn capture_performance(
    hal: &Hal,
    gpu: &crate::gpuctl::GpuController,
    node: &crate::gpu_power::Scope<'_>,
    settings: &mut ProfileSettings,
    issues: &mut Vec<String>,
) {
    settings.oem_power_mode = observe(issues, "电源模式", Some(powerctl::mode::read(hal)))
        .and_then(|mode| {
            mode.available_modes
                .contains(&mode.current_mode)
                .then_some(mode.current_mode)
        });
    settings.performance_options = observe(
        issues,
        "性能选项",
        Some(powerctl::mode::boost::capture_values(hal)),
    )
    .flatten();
    if let Some(power) = observe(issues, "CPU", Some(powerctl::read_profile(hal))) {
        settings.pl1_w = power.pl1_w;
        settings.pl2_w = power.pl2_w;
        settings.core_offset_mv = power.core_offset_mv.map(|v| v as i32);
        settings.gt_offset_mv = power.gt_offset_mv.map(|v| v as i32);
        settings.runtime_cpu = observe(issues, "即时 CPU", Some(capture_runtime(&power))).flatten();
    }
    observe(
        issues,
        "风扇",
        Some(fanctl::read_profile(hal).and_then(|fan| super::model::capture_fan(&fan, settings))),
    );
    if observe(issues, "独显连接", Some(node.synchronize(hal, gpu))).is_some() {
        settings.gpu = gpu.capture_settings().ok();
    }
}

fn capture_runtime(power: &powerctl::PowerProfile) -> Result<Option<powerctl::runtime::Settings>> {
    if !power.msr_runtime_available {
        return Ok(None);
    }
    let units = |value: Option<f64>| {
        value
            .map(msr::offset_units)
            .transpose()
            .map_err(|e| anyhow!(e))
    };
    let target = powerctl::runtime::Settings {
        pl1_w: power.runtime.pl1_w,
        pl2_w: power.runtime.pl2_w,
        core_offset_units: units(power.runtime.core_offset_mv)?,
        gt_offset_units: units(power.runtime.gt_offset_mv)?,
    };
    powerctl::runtime::validate(&target)?;
    Ok(Some(target))
}
