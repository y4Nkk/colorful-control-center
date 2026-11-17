use super::{
    output,
    store::FileStore,
    transaction::{self, Io},
};
use anyhow::{Result, bail};

fn require_battery(io: &impl Io, percent: u8) -> Result<()> {
    let (online, current) = transaction::power(io)?;
    if online || current != percent {
        bail!("供电或电量变化，已停止本次低电量亮度输出");
    }
    Ok(())
}
pub fn apply(
    io: &impl Io,
    store: &FileStore,
    expected: &str,
    notify: bool,
    permit: &mut impl FnMut() -> Result<()>,
) -> Result<String> {
    let before = transaction::snapshot(io)?;
    if !before.rule.low_supported() {
        bail!("原厂未声明低电量亮度控制");
    }
    let (_, percent) = transaction::power(io)?;
    let target = before.setting(percent)?;
    output::execute(
        io,
        store,
        expected,
        before,
        target,
        notify.then_some(2),
        permit,
        &mut || require_battery(io, percent),
    )?;
    Ok("已按原厂低电量规则更新亮度，保留首次恢复值".into())
}
pub fn finish(
    io: &impl Io,
    store: &FileStore,
    expected: &str,
    signal: bool,
    permit: &mut impl FnMut() -> Result<()>,
) -> Result<()> {
    let before = transaction::snapshot(io)?;
    let target = before.ending()?;
    let saved_percent = before.cache[1];
    output::execute(
        io,
        store,
        expected,
        before,
        target,
        signal.then_some(3),
        permit,
        &mut || {
            if !signal && !transaction::should_restore(io, saved_percent)? {
                bail!("电源或电量恢复条件已变化");
            }
            Ok(())
        },
    )
}
