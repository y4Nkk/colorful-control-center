use super::*;

pub(super) fn apply(
    io: &dyn Io,
    initial: &mut Option<Snapshot>,
    before: Snapshot,
    enabled: bool,
    mut permit: impl FnMut() -> Result<()>,
) -> Result<()> {
    permit()?;
    io.ensure_writable()?;
    if io.current()? != before {
        bail!("飞行模式前态已变化，未执行请求");
    }
    permit()?;
    initial.get_or_insert_with(|| before.clone());
    let result = (|| {
        io.firmware(enabled)?;
        permit()?;
        io.require_device(&before)?;
        io.system(&before, enabled)?;
        io.observed(&before, enabled, &mut permit)
    })();
    if let Err(error) = result {
        let restore = recover(io, &before);
        return Err(anyhow!(ccc_core::msr::with_recovery(
            format!("{error:#}"),
            restore.map_err(|e| format!("{e:#}"))
        )));
    }
    Ok(())
}
fn recover(io: &dyn Io, before: &Snapshot) -> Result<()> {
    io.require_device(before)?;
    let mut errors = vec![];
    if let Err(error) = io.firmware(before.enabled) {
        errors.push(format!("固件同步：{error:#}"));
    }
    if let Err(error) = io.system(before, before.enabled) {
        errors.push(format!("系统飞行模式：{error:#}"));
    }
    if let Err(error) = io.observed(before, before.enabled, &mut || Ok(())) {
        errors.push(format!("系统报告回读：{error:#}"));
    }
    if !errors.is_empty() {
        bail!(errors.join("；"));
    }
    Ok(())
}
