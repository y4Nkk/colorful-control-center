use super::*;
use crate::display::profile::Review;
use std::path::Path;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ManualPreview {
    pub before: Option<ProfileSettings>,
    pub mode_target: Option<powerctl::mode::Target>,
    pub display_review: Option<Review>,
    pub gpu_power_review: Option<crate::gpu_power::profile::Review>,
    pub graphics_mode_review: Option<crate::graphics_mode::profile::Review>,
    pub blocked: Option<String>,
}

pub(crate) fn preview_manual(
    hal: &Hal,
    gpu: &GpuController,
    devices: Devices<'_>,
    settings: &ProfileSettings,
) -> Result<ManualPreview> {
    crate::profiles::validate(settings)?;
    let current = crate::profiles::capture::compare_current(hal, gpu, devices, settings).ok();
    let prepared = (|| {
        let mut scopes = devices.scopes(hal, gpu)?;
        let _gate = hal.begin_write()?;
        preflight(hal, gpu, settings, &mut scopes, None).map(|(checkpoint, _)| checkpoint)
    })();
    match prepared {
        Ok(checkpoint) => Ok(ManualPreview {
            before: current,
            mode_target: checkpoint.mode.map(|plan| plan.target),
            display_review: checkpoint.display,
            gpu_power_review: checkpoint.graphics.power,
            graphics_mode_review: checkpoint.mux,
            blocked: None,
        }),
        Err(error) => Ok(ManualPreview {
            before: current,
            mode_target: None,
            display_review: None,
            gpu_power_review: None,
            graphics_mode_review: None,
            blocked: Some(format!("{error:#}")),
        }),
    }
}

#[derive(Clone, Copy)]
pub(in crate::profiles) struct Reviewed<'a> {
    pub permission: Option<&'a super::super::application::Permission>,
    pub dir: &'a Path,
    pub file: &'a super::super::ProfileFile,
    pub mode: Option<&'a powerctl::mode::Target>,
    pub display: Option<&'a Review>,
    pub gpu_power: Option<&'a crate::gpu_power::profile::Review>,
    pub graphics_mode: Option<&'a crate::graphics_mode::profile::Review>,
}
impl Reviewed<'_> {
    fn binding(&self) -> ModeReview<'_> {
        ModeReview::Confirmed {
            permission: self.permission,
            mode: self.mode,
            file: Some((self.dir, self.file)),
            display: self.display,
            gpu_power: self.gpu_power,
            graphics_mode: self.graphics_mode,
        }
    }
}
pub(in crate::profiles) enum DisplayAction {
    Begin(u64),
    Keep(u64),
}

pub(in crate::profiles) fn display_action(
    hal: &Hal,
    gpu: &GpuController,
    devices: Devices<'_>,
    reviewed: Reviewed<'_>,
    action: DisplayAction,
) -> Result<bool> {
    let _restriction =
        crate::hal::authorization::prompt::Restriction::enter(reviewed.permission.is_none());
    let mut devices = devices.scopes(hal, gpu)?;
    let _gate = hal.begin_write()?;
    check_background_software(hal, &reviewed.file.settings, reviewed.binding())?;
    let (before, _) = preflight(hal, gpu, &reviewed.file.settings, &mut devices, None)?;
    check_mode_review(&before, reviewed.binding())?;
    if reviewed.file.settings.display.is_some() != reviewed.display.is_some()
        && devices.display_owner.is_none()
    {
        bail!("档案显示尚未预览，请重新读取后确认");
    }
    match action {
        DisplayAction::Begin(id) => before.display.as_ref().map_or(Ok(false), |review| {
            devices
                .display
                .begin_profile(review, id, crate::display::now_ms())
        }),
        DisplayAction::Keep(id) => {
            devices.display.keep_profile(id, crate::display::now_ms())?;
            Ok(true)
        }
    }
}

pub(in crate::profiles) fn apply(
    hal: &Hal,
    gpu: &GpuController,
    devices: Devices<'_>,
    reviewed: Reviewed<'_>,
) -> Result<Outcome> {
    let _restriction =
        crate::hal::authorization::prompt::Restriction::enter(reviewed.permission.is_none());
    transact(
        hal,
        gpu,
        devices,
        &reviewed.file.settings,
        reviewed.binding(),
    )
}

pub(super) fn ready(devices: &Scopes<'_>, before: &Checkpoint) -> Result<()> {
    if before
        .display
        .as_ref()
        .is_some_and(|review| !review.changes().is_empty())
    {
        bail!("档案显示需要试用并明确保留，未执行其余设置");
    }
    if let Some(id) = devices.display_owner {
        devices.display.require_kept(id)?;
    }
    Ok(())
}

pub(super) fn display(settings: &ProfileSettings, devices: &Scopes<'_>) -> Result<Option<Review>> {
    settings
        .display
        .as_ref()
        .map(|target| {
            devices
                .display
                .preview_profile(target, devices.display_owner)
        })
        .transpose()
}

pub(super) fn recheck_display(settings: &ProfileSettings, devices: &Scopes<'_>) -> Result<()> {
    if let Some(current) = display(settings, devices)? {
        if !current.changes().is_empty() {
            bail!("档案显示模式已变化");
        }
    }
    if let Some(id) = devices.display_owner {
        devices.display.require_kept(id)?;
    }
    Ok(())
}
