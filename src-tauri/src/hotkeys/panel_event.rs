//! FnKey 84：OLED 唤醒与 NVSR 修复是两个独立的一次性请求。
use crate::hal::{Hal, panel_inventory::Inventory};
use anyhow::{Result, bail};
use serde::Serialize;
#[cfg(not(test))]
mod native;
#[cfg(test)]
mod tests;
pub(crate) const NVSR_SIGNATURES: [&str; 11] = [
    "FC433530", "5826BC26", "E426E270", "7D0DCB39", "E42670E2", "546414E3", "F5FA55E7", "B66E8159",
    "3F5955F9", "AAA7EA22", "0278B3AB",
];
#[derive(Clone, PartialEq, Eq, Serialize)]
struct Source {
    panels: Inventory,
    nvsr_uid: Option<u32>,
    oled: bool,
}
impl Source {
    fn identity(&self) -> Result<String> {
        super::model::digest(&("oem-panel-event-1", self))
    }
}
trait Io {
    fn current(&self) -> Result<Source>;
    fn nvsr(&self, uid: u32) -> Result<()>;
    fn oled(&self) -> Result<()>;
    fn ensure(&self) -> Result<()>;
}
pub(super) fn identity(hal: &Hal) -> Result<String> {
    #[cfg(test)]
    {
        let _ = hal;
        bail!("隔离测试未注入面板事件来源");
    }
    #[cfg(not(test))]
    {
        native::Native(hal).current()?.identity()
    }
}
pub(super) fn execute(
    hal: &Hal,
    expected: &str,
    startup: bool,
    permit: impl FnMut() -> Result<()>,
) -> Result<String> {
    #[cfg(test)]
    {
        let _ = (hal, expected, startup, permit);
        bail!("隔离测试禁止原生面板请求");
    }
    #[cfg(not(test))]
    {
        run(&native::Native(hal), expected, startup, permit)
    }
}
fn run(
    io: &impl Io,
    expected: &str,
    startup: bool,
    mut permit: impl FnMut() -> Result<()>,
) -> Result<String> {
    permit()?;
    io.ensure()?;
    let source = io.current()?;
    if source.identity()? != expected {
        bail!("面板事件来源已变化，请重新确认");
    }
    let mut errors = vec![];
    let mut completed = vec![];
    if let Some(uid) = source.nvsr_uid {
        permit()?;
        if io.current()? != source {
            bail!("NVSR请求前面板来源已变化");
        }
        permit()?;
        match io.nvsr(uid) {
            Ok(()) => completed.push("已发送NVSR修复请求"),
            Err(e) => errors.push(format!("NVSR：{e:#}")),
        }
    }
    if source.oled && !startup {
        permit()?;
        if io.current()? != source {
            bail!("OLED请求前面板来源已变化");
        }
        permit()?;
        match io.oled() {
            Ok(()) => completed.push("已发送OLED唤醒请求"),
            Err(e) => errors.push(format!("OLED：{e:#}")),
        }
    }
    if !errors.is_empty() {
        bail!("{}{}", completed.join("；"), errors.join("；"));
    }
    Ok(if completed.is_empty() {
        "无需启动修复".into()
    } else {
        completed.join("；")
    })
}
