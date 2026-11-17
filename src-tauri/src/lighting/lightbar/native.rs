use super::*;
use crate::hal::oem_dchu::Dchu;
use ccc_core::settings::{self, Patch};

pub(super) struct Native<'a> {
    dchu: &'a Dchu,
    device: hid::Device,
    kind: model::Kind,
}
fn available(dchu: &Dchu) -> Result<Vec<model::Kind>> {
    let mut kinds = model::kinds(&dchu.read_bios_capabilities(0, 24)?)?;
    if kinds.contains(&model::Kind::X58) {
        let mut packet = [0; 256];
        packet[0] = 60;
        if !x58::supported(dchu.exchange_package(4, &packet)?[0])? {
            kinds.retain(|kind| *kind != model::Kind::X58);
        }
    }
    Ok(kinds)
}
pub(super) fn open(dchu: &Dchu) -> Result<Option<Native<'_>>> {
    let kinds = available(dchu)?;
    if kinds.is_empty() {
        return Ok(None);
    }
    let device = hid::Device::open(&kinds)?;
    Ok(Some(Native {
        dchu,
        kind: device.kind(),
        device,
    }))
}
impl Io for Native<'_> {
    fn ensure_available(&self) -> Result<()> {
        crate::hal::oem_owner::ensure_owner_absent()
    }
    fn snapshot(&self) -> Result<Snapshot> {
        if !available(self.dchu)?.contains(&self.kind) {
            bail!("独立灯条型号或可用状态已变化");
        }
        Ok(Snapshot {
            kind: self.kind,
            identity: self.device.identity(),
            bytes: self
                .dchu
                .read_settings(2, 108, 19)?
                .try_into()
                .map_err(|_| anyhow!("灯条保存块长度无效"))?,
        })
    }
    fn send(&self, packet: &[u8; 64]) -> Result<()> {
        self.device.send(packet)
    }
    fn save(&self, bytes: &[u8; 19]) -> Result<()> {
        settings::apply(self.dchu, 2, &[Patch::new(108, bytes.to_vec())], None)
            .map_err(|e| anyhow!(e))?;
        if self.dchu.read_settings(2, 108, 19)? != bytes {
            bail!("灯条保存回读不一致");
        }
        Ok(())
    }
}
