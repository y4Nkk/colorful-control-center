use super::model::{Record, State};
use anyhow::{Result, bail};

pub trait Io {
    fn state(&self) -> Result<State>;
    fn record(&self) -> Result<Option<Record>>;
    fn task(&self, xml: Option<&str>) -> Result<()>;
    fn run(&self, value: Option<&str>) -> Result<()>;
    fn save(&self, record: &Option<Record>) -> Result<()>;
}
pub fn apply(
    io: &impl Io,
    expected: &State,
    record: &Option<Record>,
    target: &State,
) -> Result<()> {
    if io.state()? != *expected || io.record()? != *record {
        bail!("登录自启预览已变化，请重新读取");
    }
    let next = Some(Record {
        version: 1,
        before: record
            .as_ref()
            .map_or_else(|| expected.clone(), |r| r.before.clone()),
        applied: target.clone(),
    });
    // 先保存可恢复前态。失败时保留记录，直到系统前态与记录均确认恢复。
    io.save(&next)?;
    let result = write(io, target);
    if let Err(error) = result {
        if let Err(recovery) = write(io, expected).and_then(|_| io.save(record)) {
            bail!("登录自启保存失败：{error:#}；恢复未完成：{recovery:#}");
        }
        return Err(error);
    }
    Ok(())
}
fn write(io: &impl Io, state: &State) -> Result<()> {
    io.task(state.task.as_deref())?;
    io.run(state.run.as_deref())?;
    if io.state()? != *state {
        bail!("系统未保留登录自启设置");
    }
    Ok(())
}
