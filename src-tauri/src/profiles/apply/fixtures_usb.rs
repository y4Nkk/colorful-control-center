use super::*;

impl crate::platform::usb::Io for Device {
    fn firmware(&self) -> anyhow::Result<Option<u8>> {
        let state = self.0.lock().unwrap();
        Ok(
            (state.bios[..2] == [1, 0] && state.bios[23] & 1 != 0)
                .then(|| state.policy_tables.get(&53).map_or(0, |table| table[0])),
        )
    }
    fn power(&self) -> &dyn crate::hal::power_plan::PowerAccess {
        self
    }
    fn ensure_write(&self) -> anyhow::Result<()> {
        Ok(())
    }
}
