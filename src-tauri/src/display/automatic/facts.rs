use crate::display::{Display, Mode};
use anyhow::{Result, bail};
use ccc_core::oem_refresh::{self, Firmware};
use serde::Serialize;
use sha2::{Digest, Sha256};

#[derive(Clone)]
pub(super) struct Evidence {
    pub packet: [u8; 256],
    pub customer: u8,
    pub dc_mode_power: bool,
    pub product: u32,
    pub mode: u8,
    pub modes: Vec<u8>,
    pub fields: [u8; 3],
    pub on_ac: Option<bool>,
}
pub(super) trait Source {
    fn evidence(&self) -> Result<Evidence>;
    fn displays(&self) -> Result<Vec<Display>>;
}
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Facts {
    pub identity: String,
    pub display: Mode,
    pub candidates: Vec<u32>,
    pub low: u32,
    pub saved_or_highest: u32,
    pub fields: [u8; 3],
    pub mode: u8,
    pub modes: Vec<u8>,
    pub customer: u8,
    pub product: u32,
    pub supply_mode_switch: bool,
    pub on_ac: Option<bool>,
    pub default_enabled: bool,
}
pub(super) fn read(source: &impl Source) -> Result<Facts> {
    let evidence = source.evidence()?;
    let firmware = Firmware::from_packet(&evidence.packet).map_err(anyhow::Error::msg)?;
    if !firmware.supported || evidence.product == 4106 {
        bail!("原厂未开放此机型的自动刷新率");
    }
    if evidence.product == 0 || evidence.product > u16::MAX.into() {
        bail!("原厂机型标识未知，未开放自动刷新率");
    }
    if evidence.fields[..2].iter().any(|value| *value > 1) {
        bail!("原厂自动刷新率开关字段无效");
    }
    if evidence.mode > 4 || !evidence.modes.contains(&evidence.mode) {
        bail!("原厂当前电源模式未知或不可用");
    }
    let displays = source.displays()?;
    let mut primary = displays.into_iter().filter(|display| display.primary);
    let display = primary
        .next()
        .ok_or_else(|| anyhow::anyhow!("没有活动主显示器"))?;
    if primary.next().is_some() {
        bail!("主显示器状态不明确");
    }
    display.current.validate()?;
    let candidates = oem_refresh::candidates(&display.frequencies);
    if candidates.len() < 2 || !candidates.iter().any(|frequency| *frequency > 60) {
        bail!("主显示器没有两档已确认的原厂候选刷新率");
    }
    let low = oem_refresh::lowest(&candidates).map_err(anyhow::Error::msg)?;
    let saved_or_highest = oem_refresh::saved_or_highest(&candidates, evidence.fields[2])
        .map_err(anyhow::Error::msg)?;
    let mut stable = display.current.clone();
    stable.frequency = 0;
    stable.name.clear();
    let identity = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&(
            "oem-auto-refresh-1",
            stable,
            &candidates,
            evidence.customer,
            evidence.dc_mode_power,
            evidence.product,
            &evidence.modes,
            firmware.supported,
            firmware.default_enabled,
        ))?)
    );
    Ok(Facts {
        identity,
        display: display.current,
        candidates,
        low,
        saved_or_highest,
        fields: evidence.fields,
        mode: evidence.mode,
        modes: evidence.modes,
        customer: evidence.customer,
        product: evidence.product,
        supply_mode_switch: evidence.dc_mode_power || evidence.customer == 22,
        on_ac: evidence.on_ac,
        default_enabled: firmware.default_enabled,
    })
}
