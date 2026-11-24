use super::*;
use ccc_core::fan::OemCurvePoints;

pub(super) fn points(settings: &ProfileSettings) -> [Option<OemCurvePoints>; 3] {
    ["cpu", "gpu1", "gpu2"].map(|id| {
        settings
            .fan_curves
            .as_ref()
            .and_then(|curves| curves.get(id))
            .map(|p| OemCurvePoints {
                t2: p.t2,
                d2: p.d2_pct,
                t3: p.t3,
                d3: p.d3_pct,
            })
    })
}

fn power_patches(hal: &Hal, settings: &ProfileSettings) -> Result<(Vec<Patch>, Vec<Patch>)> {
    let controls = powerctl::CpuControls {
        pl1_w: settings.pl1_w,
        pl2_w: settings.pl2_w,
        core_offset_mv: settings.core_offset_mv,
        gt_offset_mv: settings.gt_offset_mv,
    };
    if controls.is_empty() {
        return Ok((vec![], vec![]));
    }
    let patches = powerctl::saved_patches(hal, controls)?;
    let before = if patches.is_empty() {
        Vec::new()
    } else {
        powerctl::checkpoint_patches(hal, &patches)?
    };
    Ok((patches, before))
}

#[cfg(not(test))]
fn require_oem_control(settings: &ProfileSettings) -> Result<()> {
    if settings.oem_power_mode.is_some()
        || settings.performance_options.is_some()
        || settings.display.is_some()
        || settings.power_delivery.is_some()
    {
        crate::hal::oem_owner::ensure_owner_absent()?;
    }
    Ok(())
}

pub(super) fn preflight(
    hal: &Hal,
    gpu: &GpuController,
    settings: &ProfileSettings,
    devices: &mut Scopes<'_>,
    initial: Option<&Checkpoint>,
) -> Result<(Checkpoint, Vec<Patch>)> {
    #[cfg(not(test))]
    require_oem_control(settings)?;
    let mode = crate::profiles::preview::checkpoint_mode(
        &devices.mode.context,
        settings,
        initial.and_then(|p| p.mode.as_ref()),
    )?;
    let options = options_preflight(hal, settings, mode.as_ref().map(|p| &p.core))?;
    let dust = dust_preflight(hal, devices, settings)?;
    let graphics = graphics::Plan::prepare(
        hal,
        gpu,
        devices,
        settings,
        mode.as_ref(),
        initial.map(|p| &p.graphics),
    )?;
    let mux = settings
        .graphics_mode
        .as_ref()
        .map(|target| devices.graphics_mode.preview_profile(hal, target))
        .transpose()?;
    let mux_fan = mux
        .as_ref()
        .is_some_and(|review| review.changes() && review.software_fan_running);
    let mut fan = fan_preflight(hal, settings, dust.as_ref(), graphics.deferred)?;
    if fan.is_none() && (graphics.needs_fan_snapshot() || mux_fan) {
        fan = Some(fanctl::checkpoint(hal, false)?);
    }
    let (patches, power) = power_patches(hal, settings)?;
    let runtime = settings
        .runtime_cpu
        .map(|target| powerctl::runtime::Plan::prepare(hal, target))
        .transpose()?;
    let display = review::display(settings, devices)?;
    Ok((
        Checkpoint {
            display,
            platform: platform::Plan::prepare(
                hal,
                devices,
                settings,
                mode.as_ref().map(|p| &p.core),
            )?,
            mode,
            options,
            fan,
            power,
            graphics,
            mux,
            runtime,
            dust,
            keyboard: keyboard::Plan::prepare(hal, devices, settings)?,
            touchpad: settings
                .touchpad
                .as_ref()
                .map(|target| devices.touchpad.prepare(hal, target))
                .transpose()?,
            microphones: settings
                .microphones
                .as_ref()
                .map(|target| devices.microphones.prepare(hal, target))
                .transpose()?,
            light_bar: settings
                .light_bar
                .as_ref()
                .map(|value| devices.light_bar.prepare(hal, value))
                .transpose()?,
        },
        patches,
    ))
}

fn options_preflight(
    hal: &Hal,
    settings: &ProfileSettings,
    mode: Option<&powerctl::mode::Plan>,
) -> Result<Option<powerctl::mode::boost::Plan>> {
    powerctl::mode::boost::validate_profile(
        hal,
        settings.performance_options.as_ref(),
        mode,
        super::super::model::fan_target(settings),
    )?;
    settings
        .performance_options
        .as_ref()
        .filter(|_| mode.is_none())
        .map(|values| powerctl::mode::boost::Plan::prepare(hal, values))
        .transpose()
}

fn fan_preflight(
    hal: &Hal,
    settings: &ProfileSettings,
    dust: Option<&fanctl::dust::Plan>,
    awaiting_gpu: bool,
) -> Result<Option<fanctl::FanCheckpoint>> {
    let include_curve = settings.fan_curves.is_some() || settings.fan_rpm_curves.is_some();
    if settings.fan_mode.is_none() && !include_curve && settings.fan_software_curves.is_none() {
        return Ok(None);
    }
    fanctl::ensure_manual_allowed(hal)?;
    let profile = fanctl::read_profile(hal)?;
    if let Some(curves) = &settings.fan_software_curves {
        if awaiting_gpu {
            if !dust.is_some_and(|plan| plan.disables_schedule()) {
                fanctl::dust::ensure_software_allowed(hal)?;
            }
            fanctl::software::validate_points(hal, curves)?;
        } else if dust.is_some_and(|plan| plan.disables_schedule()) {
            fanctl::software::validate_configuration(hal, curves)?;
        } else {
            fanctl::software::validate(hal, curves)?;
        }
    }
    if settings
        .fan_mode
        .is_some_and(|mode| !profile.available_modes.contains(&mode))
    {
        bail!("固件未声明支持档案中的风扇模式");
    }
    if settings.fan_curves.is_some() {
        fanctl::validate_curve(hal, points(settings))?;
    } else if let Some(curves) = &settings.fan_rpm_curves {
        fanctl::rpm::validate(hal, curves)?;
    } else if settings.fan_mode == Some(6) {
        fanctl::validate_custom_table(hal, &profile)?;
    }
    Ok(Some(fanctl::checkpoint(
        hal,
        include_curve || settings.fan_mode == Some(6),
    )?))
}

fn dust_preflight(
    hal: &Hal,
    devices: &Scopes<'_>,
    settings: &ProfileSettings,
) -> Result<Option<fanctl::dust::Plan>> {
    let software_after = settings.fan_software_curves.is_some()
        || (settings.fan_mode.is_none()
            && settings.fan_curves.is_none()
            && settings.fan_rpm_curves.is_none()
            && fanctl::software::active(hal)?);
    settings
        .dust_schedule
        .map(|schedule| devices.dust.prepare(hal, schedule, software_after))
        .transpose()
}
