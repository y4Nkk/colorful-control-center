use super::*;
use std::time::{Duration, Instant};

pub(super) struct Native<'a>(pub &'a Hal);
impl Io for Native<'_> {
    fn require_device(&self, source: &Snapshot) -> Result<()> {
        crate::hal::airplane::require_device(&source.device)
    }
    fn current(&self) -> Result<Snapshot> {
        if !self.0.reviewed_dsdt {
            bail!("当前机型的原厂飞行模式协议尚未核对");
        }
        self.0.with_dchu(|d| d.read_bios_capabilities(0, 2))?;
        crate::hal::airplane::read()
    }
    fn ensure_writable(&self) -> Result<()> {
        self.0.ensure_mode_change_allowed()?;
        crate::hal::oem_owner::ensure_owner_absent()
    }
    fn firmware(&self, enabled: bool) -> Result<()> {
        let _gate = self.0.begin_write()?;
        self.0
            .with_dchu(|d| d.set_data_sub(121, 26, u32::from(enabled)))
    }
    fn system(&self, source: &Snapshot, enabled: bool) -> Result<()> {
        self.require_device(source)?;
        crate::hal::airplane::write(&source.device, enabled)
    }
    fn observed(
        &self,
        source: &Snapshot,
        enabled: bool,
        permit: &mut dyn FnMut() -> Result<()>,
    ) -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            permit()?;
            let current = crate::hal::airplane::read()?;
            current.require_device(source)?;
            if current.enabled == enabled {
                return Ok(());
            }
            if Instant::now() >= deadline {
                bail!("等待系统飞行模式报告超时");
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}
