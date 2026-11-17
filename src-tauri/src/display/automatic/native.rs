use super::{
    facts::{self, Evidence, Source},
    fields,
};
use crate::{
    display::{Controller, Display},
    hal::Hal,
};
use anyhow::{Result, bail};

pub(super) struct Native<'a> {
    pub hal: &'a Hal,
    pub display: &'a Controller,
}
impl Source for Native<'_> {
    fn evidence(&self) -> Result<Evidence> {
        let (packet, model, mode, modes, fields) = self.hal.with_dchu(|io| {
            let packet = io.bios_capabilities()?;
            let model = crate::hal::oem_identity::read(io)?;
            let mode = io.read_settings(1, 1, 1)?[0];
            let modes = crate::powerctl::mode::available(io)?;
            let fields = io
                .read_settings(1, 27, 3)?
                .try_into()
                .map_err(|_| anyhow::anyhow!("刷新率字段长度无效"))?;
            Ok((packet, model, mode, modes, fields))
        })?;
        Ok(Evidence {
            packet,
            customer: model.customer,
            dc_mode_power: model.dc_mode_power,
            product: model.product,
            mode,
            modes,
            fields,
            on_ac: crate::battery::system_power()?.ac_online,
        })
    }
    fn displays(&self) -> Result<Vec<Display>> {
        Ok(self.display.read()?.displays)
    }
}
impl fields::Io for Native<'_> {
    fn identity(&self) -> Result<String> {
        Ok(facts::read(self)?.identity)
    }
    fn fields(&self) -> Result<[u8; 3]> {
        self.hal.with_dchu(|io| {
            io.read_settings(1, 27, 3)?
                .try_into()
                .map_err(|_| anyhow::anyhow!("刷新率字段长度无效"))
        })
    }
    fn write(&self, index: usize, value: u8) -> Result<()> {
        if index >= 3 || index < 2 && value > 1 {
            bail!("自动刷新率字段无效");
        }
        self.hal
            .with_dchu(|io| io.write_settings(1, 27 + index as i32, &[value]))
    }
}
