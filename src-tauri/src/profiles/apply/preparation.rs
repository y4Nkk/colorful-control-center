use super::*;

pub(super) fn transact(
    hal: &Hal,
    gpu: &GpuController,
    devices: Devices<'_>,
    settings: &ProfileSettings,
    review: ModeReview<'_>,
) -> Result<Outcome> {
    check_background_software(hal, settings, review)?;
    let mut devices = devices.scopes(hal, gpu)?;
    let mut initial = if settings.perkey_lighting.is_some()
        || settings.keymap.is_some()
        || settings.lan_policy.is_some()
        || settings.gpu_power.is_some()
        || settings.oem_power_mode.is_some()
    {
        let _gate = hal.begin_write()?;
        let (before, _) = preflight(hal, gpu, settings, &mut devices, None)?;
        check_mode_review(&before, review)?;
        review::ready(&devices, &before)?;
        Some(before)
    } else {
        None
    };
    let mut prefix = Vec::new();
    let setup = initial.as_mut().map_or(Ok(()), |before| {
        prepare(
            hal,
            gpu,
            settings,
            &mut devices,
            before,
            review,
            &mut prefix,
        )
    });
    let result = setup
        .and_then(|_| transact_locked(hal, gpu, settings, &mut devices, review, initial.as_ref()));
    let mut result = recover_setup(
        hal,
        gpu,
        &mut devices,
        initial.as_ref(),
        result,
        &mut prefix,
    );
    if let Ok(outcome) = &mut result {
        prefix.append(&mut outcome.steps);
        outcome.steps = prefix;
    }
    finish_sessions(&mut devices, &mut result);
    result
}
fn prepare(
    hal: &Hal,
    gpu: &GpuController,
    settings: &ProfileSettings,
    devices: &mut Scopes<'_>,
    initial: &mut Checkpoint,
    review: ModeReview<'_>,
    steps: &mut Vec<ApplyStep>,
) -> Result<()> {
    check_authority(review)?;
    initial.keyboard.prepare_sessions(hal, devices)?;
    check_authority(review)?;
    if settings.lan_policy.is_some() {
        initial.platform.authorize(devices)?;
    }
    check_authority(review)?;
    initial.graphics.authorize(devices, initial.mode.as_ref())?;
    check_authority(review)?;
    if initial
        .graphics
        .stages(initial.mode.as_ref())?
        .is_some_and(|graph| graph.preparation.is_some())
    {
        let _gate = hal.begin_write()?;
        let (before, _) = preflight(hal, gpu, settings, devices, None)?;
        check_mode_review(&before, review)?;
        review::ready(devices, &before)?;
        if initial.mode.as_ref().map(|p| &p.target) != before.mode.as_ref().map(|p| &p.target)
            || initial.graphics.power != before.graphics.power
        {
            bail!("档案准备前完整目标已变化，请重新预览");
        }
        initial.graphics = before.graphics;
        stage(
            steps,
            "开启独显与策略",
            initial
                .graphics
                .enable(devices, initial.mode.as_ref(), &mut || {
                    check_authority(review)
                }),
        )?;
    }
    Ok(())
}
fn recover_setup(
    hal: &Hal,
    gpu: &GpuController,
    devices: &mut Scopes<'_>,
    initial: Option<&Checkpoint>,
    result: Result<Outcome>,
    steps: &mut Vec<ApplyStep>,
) -> Result<Outcome> {
    let Err(error) = result else {
        return result;
    };
    let primary_reported = steps.iter().any(|s| !s.ok);
    if let Some(before) = initial {
        let _gate = hal.begin_write()?;
        before.graphics.rollback(gpu, devices, false, steps);
    }
    if steps.is_empty() {
        return Err(error);
    }
    if !primary_reported {
        let _ = stage(steps, "档案开启后复核", Err(error));
    }
    Ok(Outcome {
        steps: Vec::new(),
        targets_applied: false,
    })
}
fn finish_sessions(devices: &mut Scopes<'_>, result: &mut Result<Outcome>) {
    let success = result.as_ref().is_ok_and(|outcome| outcome.targets_applied);
    for (name, finish) in [
        ("恢复逐键按键响应", devices.perkey.finish(success)),
        ("恢复程序宏监听", devices.keymap.finish(success)),
        ("网络授权收尾", devices.lan.finish().map(|_| false)),
        ("独显授权收尾", devices.mode.finish().map(|_| false)),
    ] {
        match &mut *result {
            Ok(outcome) => match finish {
                Ok(true) => {
                    let _ = stage(&mut outcome.steps, name, Ok(()));
                }
                Err(error) => {
                    let _ = stage(&mut outcome.steps, name, Err(error));
                }
                _ => (),
            },
            Err(error) => {
                if let Err(recovery) = finish {
                    *error = anyhow::anyhow!(ccc_core::msr::with_recovery(
                        format!("{error:#}"),
                        Err(format!("{name}：{recovery:#}")),
                    ));
                }
            }
        }
    }
}
