use super::model::digest;
use anyhow::{Result, bail};
use std::path::PathBuf;

#[cfg(not(test))]
mod native;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Source {
    path: PathBuf,
    fingerprint: String,
}
impl Source {
    fn identity(&self) -> Result<String> {
        digest(&("windows-recovery-1", &self.path, &self.fingerprint))
    }
}
trait Io {
    fn source(&self) -> Result<Source>;
    fn open(&self, source: &Source) -> Result<()>;
}
fn run(io: &impl Io, expected: &str, mut permit: impl FnMut() -> Result<()>) -> Result<()> {
    permit()?;
    let source = io.source()?;
    if source.identity()? != expected {
        bail!("Windows恢复入口已变化，请重新确认");
    }
    permit()?;
    if io.source()? != source {
        bail!("Windows恢复程序在执行前发生变化");
    }
    permit()?;
    io.open(&source)
}
pub(super) fn identity() -> Result<String> {
    #[cfg(test)]
    {
        bail!("隔离测试未注入Windows恢复入口");
    }
    #[cfg(not(test))]
    {
        native::Native.source()?.identity()
    }
}
pub(super) fn execute(expected: &str, permit: impl FnMut() -> Result<()>) -> Result<()> {
    #[cfg(test)]
    {
        let _ = (expected, permit);
        bail!("隔离测试不启动系统恢复程序");
    }
    #[cfg(not(test))]
    {
        run(&native::Native, expected, permit)
    }
}

#[cfg(test)]
mod tests;
