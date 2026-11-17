use super::*;

pub(super) struct Native<'a>(pub &'a Hal);
impl Io for Native<'_> {
    fn snapshot(&self) -> Result<Snapshot> {
        let mut endpoints = crate::hal::audio::capture()?;
        endpoints.sort_by(|a, b| a.id.cmp(&b.id));
        let saved = self.0.with_dchu(|io| Ok(io.read_settings(1, 41, 1)?[0]))?;
        let snapshot = Snapshot { endpoints, saved };
        snapshot.validate()?;
        Ok(snapshot)
    }
    fn ensure_available(&self) -> Result<()> {
        crate::hal::oem_owner::ensure_owner_absent()
    }
    fn set_muted(&self, id: &str, muted: bool) -> Result<()> {
        crate::hal::audio::set_muted(id, muted)
    }
    fn save(&self, value: u8) -> Result<()> {
        self.0.with_dchu(|io| {
            ccc_core::settings::apply(io, 1, &[ccc_core::settings::Patch::new(41, [value])], None)
                .map(|_| ())
                .map_err(anyhow::Error::msg)
        })
    }
}
