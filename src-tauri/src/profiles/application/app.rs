use super::*;

pub(crate) fn apply_startup(
    app: &tauri::AppHandle,
    hal: &Hal,
    gpu: &GpuController,
    file: &ProfileFile,
    approved_mode: Option<&powerctl::mode::profile::Grant>,
    permission: Permission,
) -> Result<Application> {
    permission.check()?;
    let reviews = Fresh::read(app, hal, file)?;
    reviews.check(approved_mode)?;
    begin_fresh(
        app,
        hal,
        gpu,
        file,
        Origin::Startup,
        Some(permission),
        reviews,
    )
}

pub(in crate::profiles) fn apply_automatic(
    app: &tauri::AppHandle,
    file: &ProfileFile,
    approved_mode: Option<&powerctl::mode::profile::Grant>,
    origin: Origin,
    permission: Permission,
) -> Result<Application> {
    permission.check()?;
    let hal = app.state::<Hal>();
    let gpu = app.state::<GpuController>();
    let reviews = Fresh::read(app, &hal, file)?;
    reviews.check(approved_mode)?;
    begin_fresh(app, &hal, &gpu, file, origin, Some(permission), reviews)
}

struct Fresh {
    approval: Option<powerctl::mode::profile::ApprovalPreview>,
    mode: Option<powerctl::mode::Target>,
    display: Option<display::profile::Review>,
    gpu_power: Option<crate::gpu_power::profile::Review>,
    graphics_mode: Option<crate::graphics_mode::profile::Review>,
}
impl Fresh {
    fn check(&self, approved: Option<&powerctl::mode::profile::Grant>) -> Result<()> {
        match (self.approval.as_ref(), approved) {
            (Some(actual), Some(approved)) => actual.check(approved),
            (None, None) => Ok(()),
            _ => bail!("后台档案模式联动未获得完整确认"),
        }
    }
    fn read(app: &tauri::AppHandle, hal: &Hal, file: &ProfileFile) -> Result<Self> {
        let power_controller = app.state::<crate::gpu_power::Controller>();
        let gpu = app.state::<GpuController>();
        let mode_session =
            powerctl::mode::links::Context::native(hal, &gpu, &power_controller).session()?;
        let mux_controller = app.state::<crate::graphics_mode::Controller>();
        let mux_scope = mux_controller.scope()?;
        let (approval, display, gpu_power, graphics_mode) = {
            let _gate = hal.begin_write()?;
            (
                crate::profiles::preview_authorization(&mode_session.context, &file.settings)?,
                file.settings
                    .display
                    .as_ref()
                    .map(|target| {
                        app.state::<display::Controller>()
                            .preview_profile(target, None)
                    })
                    .transpose()?,
                file.settings
                    .gpu_power
                    .as_ref()
                    .map(|target| mode_session.node_scope().preview_profile(hal, target))
                    .transpose()?,
                file.settings
                    .graphics_mode
                    .as_ref()
                    .map(|target| mux_scope.preview_profile(hal, target))
                    .transpose()?,
            )
        };
        drop(mux_scope);
        drop(mode_session);
        Ok(Self {
            mode: approval.as_ref().map(|approval| approval.target.clone()),
            approval,
            display,
            gpu_power,
            graphics_mode,
        })
    }
}
fn begin_fresh(
    app: &tauri::AppHandle,
    hal: &Hal,
    gpu: &GpuController,
    file: &ProfileFile,
    origin: Origin,
    permission: Option<Permission>,
    reviews: Fresh,
) -> Result<Application> {
    let dir = crate::profiles::profiles_dir(app)?;
    app.state::<Controller>().begin(
        Runtime {
            origin,
            permission,
            hal,
            gpu,
            devices: Devices::from_app(app),
            dir: &dir,
        },
        file,
        reviews.mode.as_ref(),
        reviews.display.as_ref(),
        reviews.gpu_power.as_ref(),
        reviews.graphics_mode.as_ref(),
    )
}

pub fn apply_preview(
    app: &tauri::AppHandle,
    hal: &Hal,
    gpu: &GpuController,
    file: &ProfileFile,
    mode: Option<&powerctl::mode::Target>,
    display: Option<&display::profile::Review>,
    gpu_power: Option<&crate::gpu_power::profile::Review>,
    graphics_mode: Option<&crate::graphics_mode::profile::Review>,
) -> Result<Application> {
    let dir = crate::profiles::profiles_dir(app)?;
    app.state::<Controller>().begin(
        Runtime {
            origin: Origin::Manual,
            permission: None,
            hal,
            gpu,
            devices: Devices::from_app(app),
            dir: &dir,
        },
        file,
        mode,
        display,
        gpu_power,
        graphics_mode,
    )
}
pub fn keep(app: &tauri::AppHandle, id: u64) -> Result<Application> {
    let result = app.state::<Controller>().keep(
        &app.state::<Hal>(),
        &app.state::<GpuController>(),
        Devices::from_app(app),
        id,
    )?;
    crate::profiles::application::log_application(&result);
    Ok(result)
}
pub fn cancel(app: &tauri::AppHandle, id: u64) -> Result<Application> {
    let result = app.state::<Controller>().cancel(
        &app.state::<Hal>(),
        &app.state::<display::Controller>(),
        id,
    )?;
    crate::profiles::application::log_application(&result);
    Ok(result)
}
