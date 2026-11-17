use super::*;

pub(super) fn execute(
    hal: &Hal,
    devices: &mut Scopes<'_>,
    review: Option<&crate::graphics_mode::profile::Review>,
    fan_touched: &mut bool,
    steps: &mut Vec<ApplyStep>,
) -> Result<()> {
    let Some(review) = review else {
        return Ok(());
    };
    let resume = if review.changes() && fanctl::software::read(hal)?.is_some() {
        let checkpoint = fanctl::checkpoint(hal, false)?;
        *fan_touched = true;
        stage(
            steps,
            "显卡选择前释放软件风扇",
            fanctl::software::stop_safely(hal),
        )?;
        Some(checkpoint)
    } else {
        None
    };
    stage(
        steps,
        "显卡模式选择请求（重启后核对）",
        devices.graphics_mode.execute_profile(hal, review),
    )?;
    if let Some(checkpoint) = resume {
        stage(
            steps,
            "恢复本次软件风扇",
            fanctl::restore_checkpoint(hal, &checkpoint),
        )?;
    }
    Ok(())
}

pub(super) fn rollback(
    hal: &Hal,
    devices: &mut Scopes<'_>,
    fan_touched: &mut bool,
    steps: &mut Vec<ApplyStep>,
) {
    if !devices.graphics_mode.profile_dirty() {
        return;
    }
    if fanctl::software::read(hal).ok().flatten().is_some() {
        *fan_touched = true;
        if stage(
            steps,
            "恢复显卡选择前释放软件风扇",
            fanctl::software::stop_safely(hal),
        )
        .is_err()
        {
            return;
        }
    }
    let _ = stage(
        steps,
        "恢复本次显卡选择（重启后核对）",
        devices.graphics_mode.rollback_profile(hal),
    );
}
