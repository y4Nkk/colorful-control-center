use super::{Io, model::Source, resource::Resource};
use crate::hal::{Hal, oem_dchu::Dchu};
use anyhow::Result;

pub(super) struct Native<'a>(pub &'a Hal);
fn flags(dchu: &Dchu) -> Result<[u8; 256]> {
    dchu.bios_capabilities()
}
impl Io for Native<'_> {
    fn current(&self) -> Result<Source> {
        let inventory = self.0.panel_inventory()?;
        if inventory.monitors.iter().any(|m| m.oled) {
            return Source::new(inventory, None, Some(system::saver()?));
        }
        if inventory.monitors.is_empty() || !inventory.unresolved.is_empty() {
            return Source::new(inventory, None, None);
        }
        let flags = {
            let _gate = self.0.begin_write()?;
            self.0.with_dchu(flags)?
        };
        Source::new(inventory, Some(&flags), None)
    }
    fn prepare(&self, source: &Source) -> Result<Box<dyn Resource>> {
        system::prepare(source)
    }
    fn wake(&self, source: &Source) -> Result<()> {
        self.current()?.require(&source.identity()?)?;
        let _gate = self.0.begin_write()?;
        self.0.with_dchu(|dchu| {
            let flags = flags(dchu)?;
            Source::new(source.inventory.clone(), Some(&flags), None)?
                .require(&source.identity()?)?;
            dchu.set_u32(94, 1)
        })
    }
}

#[cfg(not(test))]
#[path = "native_system.rs"]
mod system;
#[cfg(test)]
mod system {
    use super::*;
    pub(super) fn saver() -> Result<super::super::model::Saver> {
        anyhow::bail!("隔离测试禁止查询系统屏保文件")
    }
    pub(super) fn prepare(_: &Source) -> Result<Box<dyn Resource>> {
        anyhow::bail!("隔离测试禁止启动真实屏保或输入监听")
    }
}
