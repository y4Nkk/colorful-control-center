use super::oem_dchu::Dchu;
use anyhow::{Result, anyhow};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Model {
    pub product: u32,
    pub customer: u8,
    pub dc_mode_power: bool,
}
fn decode(info: &[u8; 256], saved: [u8; 4]) -> Model {
    Model {
        product: ccc_core::oem_identity::product_id(saved),
        customer: info[2],
        dc_mode_power: info[13] & 0x80 != 0,
    }
}
pub(crate) fn read(io: &Dchu) -> Result<Model> {
    let info = io.read_package(17)?;
    let saved = io
        .read_settings(0, 2, 4)?
        .try_into()
        .map_err(|_| anyhow!("机型标识长度无效"))?;
    Ok(decode(&info, saved))
}

#[cfg(test)]
mod tests {
    #[test]
    fn original_information_packet_and_saved_id_are_decoded_together() {
        let mut info = [0; 256];
        info[2] = 7;
        info[13] = 0x80;
        let model = super::decode(&info, [0x40, 0xc1, 0, 0]);
        assert_eq!(
            (model.product, model.customer, model.dc_mode_power),
            (16577, 7, true)
        );
    }
}
