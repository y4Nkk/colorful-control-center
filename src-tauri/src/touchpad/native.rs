use super::*;
pub(super) struct Native<'a>(pub &'a Hal);
impl Io for Native<'_> {
    fn snapshot(&self) -> Result<Snapshot> {
        let (devices, enabled) = crate::hal::touchpad::capture()?;
        let saved = self.0.with_dchu(|io| Ok(io.read_settings(1, 10, 1)?[0]))?;
        Ok(Snapshot {
            devices,
            enabled,
            saved,
        })
    }
    fn ensure_available(&self) -> Result<()> {
        crate::hal::oem_owner::ensure_owner_absent()
    }
    fn set_enabled(&self, devices: &[String], before: bool, enabled: bool) -> Result<()> {
        crate::hal::touchpad::set_enabled(devices, before, enabled)
    }
    fn save(&self, value: u8) -> Result<()> {
        self.0.with_dchu(|io| {
            ccc_core::settings::apply(io, 1, &[ccc_core::settings::Patch::new(10, [value])], None)
                .map(|_| ())
                .map_err(anyhow::Error::msg)
        })
    }
}
