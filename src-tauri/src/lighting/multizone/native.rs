use super::*;
use crate::hal::oem_dchu::Dchu;

pub(super) fn supported(dchu: &Dchu) -> Result<bool> {
    let raw = dchu.read_package(13)?[15];
    let cache = dchu.read_settings(2, 4, 1)?.first().copied();
    Ok(crate::lighting::select_detection(Some(raw), cache).0 == Some(23))
}
pub(super) fn decode(flags: &[u8], status: Option<u8>) -> Result<Capabilities> {
    if flags.len() != 24 || flags[..2] != [1, 0] {
        bail!("多区键盘扩展能力页版本或长度未知");
    }
    let light_bar_switchable = flags[23] & 128 != 0;
    let light_bar_enabled = match (light_bar_switchable, status) {
        (false, None) => true,
        (true, Some(0)) => false,
        (true, Some(1)) => true,
        _ => bail!("多区键盘平台灯条状态未知"),
    };
    Ok(Capabilities {
        keyboard_type: 23,
        light_bar_switchable,
        light_bar_enabled,
        cover: flags[23] & 32 != 0,
    })
}
pub(super) struct Native<'a>(pub &'a Dchu);
impl Io for Native<'_> {
    fn snapshot(&self) -> Result<Snapshot> {
        if !supported(self.0)? {
            bail!("原厂type23多区键盘已不可用");
        }
        let flags = self.0.read_bios_capabilities(0, 24)?;
        if flags.len() != 24 || flags[..2] != [1, 0] {
            bail!("多区键盘扩展能力页版本或长度未知");
        }
        let status = if flags[23] & 128 != 0 {
            let mut request = [0; 256];
            request[0] = 60;
            Some(self.0.exchange_package(4, &request)?[0])
        } else {
            None
        };
        let snapshot = Snapshot {
            capabilities: decode(&flags, status)?,
            page: self.0.read_settings(2, 0, 256)?,
        };
        snapshot.validate()?;
        Ok(snapshot)
    }
    fn ensure_available(&self) -> Result<()> {
        crate::hal::oem_owner::ensure_owner_absent()
    }
    fn send(&self, packet: &[u8; 37]) -> Result<()> {
        let mut request = [0; 256];
        request[..37].copy_from_slice(packet);
        self.0.exchange_package(4, &request)?;
        Ok(())
    }
    fn save(&self, patches: &[Patch]) -> Result<()> {
        ccc_core::settings::apply(self.0, 2, patches, None).map_err(|error| anyhow!(error))?;
        Ok(())
    }
}
