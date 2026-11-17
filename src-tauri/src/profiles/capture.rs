use super::{ProfileSettings, validate};
use crate::{fanctl, hal::Hal, powerctl};
use anyhow::{Result, anyhow};
use ccc_core::msr;

pub(crate) fn capture_current(
    hal: &Hal,
    gpu: &crate::gpuctl::GpuController,
    devices: super::Devices<'_>,
) -> Result<ProfileSettings> {
    let mut devices = devices.scopes(hal, gpu)?;
    let _write = hal.begin_write()?;
    let gpu_power = devices.mode.node_scope().capture(hal)?;
    devices.mode.node_scope().synchronize(hal, gpu)?;
    let mode = powerctl::mode::read(hal)?;
    let fan = fanctl::read_profile(hal)?;
    let power = powerctl::read_profile(hal)?;
    let (battery_charge, battery_energy) = devices.battery.capture(hal)?;
    let mut settings = ProfileSettings {
        oem_power_mode: mode
            .available_modes
            .contains(&mode.current_mode)
            .then_some(mode.current_mode),
        performance_options: powerctl::mode::boost::capture_values(hal)?,
        pl1_w: power.pl1_w,
        pl2_w: power.pl2_w,
        core_offset_mv: power.core_offset_mv.map(|v| v as i32),
        gt_offset_mv: power.gt_offset_mv.map(|v| v as i32),
        runtime_cpu: capture_runtime(&power)?,
        gpu: gpu.capture_settings().ok(),
        light_bar: devices.light_bar.capture(hal)?,
        dust_schedule: devices.dust.capture(hal)?,
        keyboard: devices.keyboard.capture(hal)?,
        microphones: devices.microphones.capture(hal)?,
        touchpad: devices.touchpad.capture(hal)?,
        keyboard_lighting: devices.lighting.capture(hal)?,
        multi_zone_lighting: devices.multi_zone.capture(hal)?,
        lighting_options: devices.lighting_options.capture(hal)?,
        perkey_lighting: devices.perkey.capture(hal)?,
        keymap: devices.keymap.capture(hal)?,
        power_delivery: devices.power_delivery.capture(hal)?,
        usb_policy: devices.usb.capture(hal)?,
        battery_charge,
        battery_energy,
        lan_policy: devices.lan.capture(hal)?,
        display: devices.display.capture()?,
        gpu_power,
        graphics_mode: devices.graphics_mode.capture(hal)?,
        ..Default::default()
    };
    super::model::capture_fan(&fan, &mut settings)?;
    validate(&settings)?;
    Ok(settings)
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
