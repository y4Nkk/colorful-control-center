use super::*;
use crate::powerctl::{self, mode::links};

#[cfg(test)]
pub(crate) fn preview_mode(
    context: &links::Context<'_>,
    settings: &ProfileSettings,
) -> Result<Option<powerctl::mode::Target>> {
    Ok(checkpoint_mode(context, settings, None)?.map(|plan| plan.target))
}

pub(crate) fn preview_authorization(
    context: &links::Context<'_>,
    settings: &ProfileSettings,
) -> Result<Option<powerctl::mode::profile::ApprovalPreview>> {
    checkpoint_mode(context, settings, None)?
        .map(|plan| {
            powerctl::mode::profile::authorization(
                context,
                plan,
                settings.gpu_power.as_ref(),
                settings.gpu.is_some() || settings.fan_software_curves.is_some(),
            )
        })
        .transpose()
}

pub(super) fn checkpoint_mode(
    context: &links::Context<'_>,
    settings: &ProfileSettings,
    initial: Option<&links::Plan>,
) -> Result<Option<links::Plan>> {
    validate(settings)?;
    settings
        .oem_power_mode
        .map(|mode| {
            let plan = match initial {
                Some(initial) => powerctl::mode::profile::rebase(context, initial)?,
                None => powerctl::mode::profile::checkpoint(
                    context,
                    mode,
                    settings.performance_options.as_ref(),
                    settings.gpu_power.as_ref(),
                    settings.gpu.is_some() || settings.fan_software_curves.is_some(),
                )?,
            };
            if let Some(fan) = model::fan_target(settings) {
                powerctl::mode::boost::validate_active_fan(plan.target.turbo, fan)?;
            }
            Ok(plan)
        })
        .transpose()
}
