use super::*;

pub(super) fn transact_locked(
    hal: &Hal,
    gpu: &GpuController,
    settings: &ProfileSettings,
    devices: &mut Scopes<'_>,
    review: ModeReview<'_>,
    initial: Option<&Checkpoint>,
) -> Result<Outcome> {
    let _write = hal.begin_write()?;
    check_background_software(hal, settings, review)?;
    let (before, patches) = preflight(hal, gpu, settings, devices, initial)?;
    check_mode_review(&before, review)?;
    review::ready(devices, &before)?;
    if before.graphics.deferred {
        bail!("独显尚未开启，不能跳过驱动与温度复核");
    }
    if let Some(runtime) = &before.runtime {
        runtime.remember_baseline(hal)?;
    }
    let mut steps = Vec::new();
    let mut touched = Touched::default();
    let result = execute_targets(
        hal,
        gpu,
        settings,
        devices,
        &before,
        &patches,
        &mut touched,
        &mut steps,
        review,
    )
    .and_then(|_| {
        if !review.background() && settings.fan_software_curves.is_some() {
            stage(
                &mut steps,
                "软件曲线恢复确认",
                hal.acknowledge_software_fan_restart(),
            )?;
        }
        Ok(())
    });
    if result.is_err() {
        rollback(hal, gpu, devices, &before, &mut touched, &mut steps);
    }
    Ok(Outcome {
        steps,
        targets_applied: result.is_ok(),
    })
}

fn finish_targets(
    hal: &Hal,
    gpu: &GpuController,
    settings: &ProfileSettings,
    devices: &mut Scopes<'_>,
    before: &Checkpoint,
    touched: &mut Touched,
    steps: &mut Vec<ApplyStep>,
    review: ModeReview<'_>,
) -> Result<()> {
    guard(steps, review)?;
    before
        .graphics
        .offsets(gpu, settings, &mut touched.gpu, steps)?;
    guard(steps, review)?;
    before.graphics.close(
        hal,
        devices,
        before.mode.as_ref(),
        &mut touched.fan,
        steps,
        &mut || check_authority(review),
    )?;
    guard(steps, review)?;
    mux::execute(hal, devices, before.mux.as_ref(), &mut touched.fan, steps)?;
    if settings.gpu_power.is_some() || before.mode.is_some() {
        stage(
            steps,
            "独显目标复核",
            before.graphics.recheck(hal, devices, before.mode.as_ref()),
        )?;
    }
    if settings.display.is_some() {
        stage(
            steps,
            "显示刷新率复核",
            review::recheck_display(settings, devices),
        )?;
    }
    if settings
        .perkey_lighting
        .as_ref()
        .is_some_and(|target| target.values.enabled && target.values.pattern.reactive())
    {
        guard(steps, review)?;
        stage(steps, "逐键按键响应", devices.perkey.arm())?;
    }
    guard(steps, review)?;
    before.keyboard.commit(hal, devices, steps)?;
    Ok(())
}

fn apply_controls(
    hal: &Hal,
    devices: &Scopes<'_>,
    settings: &ProfileSettings,
    before: &Checkpoint,
    patches: &[Patch],
    steps: &mut Vec<ApplyStep>,
    touched: &mut Touched,
    review: ModeReview<'_>,
) -> Result<()> {
    if let Some(mode) = &before.mode {
        guard(steps, review)?;
        let result = execute_mode(devices, settings, mode, review);
        touched.mode = mode.core.attempted();
        touched.fan = touched.mode && before.fan.is_some();
        touched.runtime = touched.mode && before.runtime.is_some();
        stage(steps, "OEM 电源模式与联动", result)?;
        if settings.performance_options.is_some() {
            stage(steps, "性能选项", Ok(()))?;
        }
    }
    if before
        .graphics
        .stages(before.mode.as_ref())?
        .is_some_and(|graph| graph.finish.is_some_and(|r| r.target.enabled))
    {
        stage(
            steps,
            "档案最终独显策略",
            before
                .graphics
                .finish(devices, before.mode.as_ref(), true, &mut || {
                    check_authority(review)
                }),
        )?;
    }
    if let Some(options) = &before.options {
        guard(steps, review)?;
        touched.runtime |= options.changes_runtime() && before.runtime.is_some();
        stage(
            steps,
            "性能选项",
            options.execute(
                hal,
                crate::profiles::model::fan_target(settings).is_none(),
                &mut touched.options,
            ),
        )?;
    }
    if before.fan.is_some() && crate::profiles::model::fan_target(settings).is_some() {
        guard(steps, review)?;
        touched.fan = true;
        apply_fan(hal, settings, steps, review)?;
    }
    if !patches.is_empty() {
        guard(steps, review)?;
        touched.saved = true;
        stage(
            steps,
            "功耗与电压已保存",
            powerctl::apply_patches(hal, patches),
        )?;
    }
    if let Some(runtime) = &before.runtime {
        guard(steps, review)?;
        touched.runtime = true;
        stage(steps, "即时 CPU 调校", runtime.execute(hal))?;
    }
    Ok(())
}

fn execute_mode(
    devices: &Scopes<'_>,
    settings: &ProfileSettings,
    mode: &powerctl::mode::links::Plan,
    review: ModeReview<'_>,
) -> Result<()> {
    let mut checked = None;
    mode.execute(
        &devices.mode,
        &mut |_, _, identity| {
            if let ModeReview::Confirmed {
                permission: Some(permission),
                ..
            } = review
            {
                if checked.is_none() {
                    checked =
                        crate::profiles::preview_authorization(&devices.mode.context, settings)?;
                }
                let facts = checked
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("后台档案GPU模式授权缺失"))?;
                if facts
                    .clocks
                    .driver
                    .as_ref()
                    .map(|driver| driver.identity.as_str())
                    != Some(identity)
                {
                    bail!("后台档案的写前驱动与完整供电目标预览不一致");
                }
                permission.bind_mode_driver(facts)?;
            }
            Ok(())
        },
        &mut || check_authority(review),
    )
}

fn apply_fan(
    hal: &Hal,
    settings: &ProfileSettings,
    steps: &mut Vec<ApplyStep>,
    review: ModeReview<'_>,
) -> Result<()> {
    guard(steps, review)?;
    if let Some(curves) = &settings.fan_software_curves {
        check_background_software(hal, settings, review)?;
        let result = if review.background() {
            fanctl::software::apply(hal, curves)
        } else {
            fanctl::software::apply_manual(hal, curves)
        };
        return stage(steps, "软件风扇曲线", result);
    }
    if settings.fan_curves.is_some() {
        let p = points(settings);
        stage(
            steps,
            "风扇曲线",
            fanctl::apply_curve(hal, p[0], p[1], p[2]),
        )?;
    }
    guard(steps, review)?;
    if let Some(curves) = &settings.fan_rpm_curves {
        stage(steps, "十级 RPM 曲线", fanctl::rpm::apply(hal, curves))?;
    }
    guard(steps, review)?;
    if let Some(mode) = settings.fan_mode {
        if mode != 6 || (settings.fan_curves.is_none() && settings.fan_rpm_curves.is_none()) {
            stage(steps, "风扇模式", fanctl::set_mode(hal, mode))?;
        }
    }
    Ok(())
}

fn execute_targets(
    hal: &Hal,
    gpu: &GpuController,
    settings: &ProfileSettings,
    devices: &mut Scopes<'_>,
    before: &Checkpoint,
    patches: &[Patch],
    touched: &mut Touched,
    steps: &mut Vec<ApplyStep>,
    review: ModeReview<'_>,
) -> Result<()> {
    guard(steps, review)?;
    dust_schedule(hal, devices, before, touched, steps, true, review)?;
    apply_controls(
        hal, devices, settings, before, patches, steps, touched, review,
    )?;
    before
        .platform
        .execute(hal, devices, &mut touched.platform, steps, review)?;
    guard(steps, review)?;
    dust_schedule(hal, devices, before, touched, steps, false, review)?;
    apply_device_controls(hal, devices, before, touched, steps, review)?;
    finish_targets(hal, gpu, settings, devices, before, touched, steps, review)?;
    guard(steps, review)?;
    Ok(())
}

fn dust_schedule(
    hal: &Hal,
    devices: &mut Scopes<'_>,
    before: &Checkpoint,
    touched: &mut Touched,
    steps: &mut Vec<ApplyStep>,
    disabling: bool,
    review: ModeReview<'_>,
) -> Result<()> {
    if let Some(plan) = before
        .dust
        .as_ref()
        .filter(|plan| plan.disables_schedule() == disabling)
    {
        guard(steps, review)?;
        stage(
            steps,
            "除尘计划",
            devices.dust.execute(hal, plan, &mut touched.dust),
        )?;
    }
    Ok(())
}
