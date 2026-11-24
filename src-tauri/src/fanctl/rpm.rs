//! 十级 RPM 控制：全量预读、只写目标表、固件回读及原始字节恢复。
#[cfg(test)]
mod tests;
mod transaction;
use crate::hal::{Hal, oem_dchu::Dchu};
use anyhow::{Result, anyhow, bail};
use ccc_core::{
    fan::decode_oem_fan_mode,
    rpm::{self, LEVELS, Pair, RpmPoint},
    settings::{self, Patch, SettingsAccess},
};
use serde::Serialize;
use std::collections::BTreeMap;

pub type Curves = BTreeMap<String, [RpmPoint; LEVELS]>;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RpmFan {
    pub id: String,
    pub maximum_rpm: u16,
    pub points: [RpmPoint; LEVELS],
}

#[derive(Clone)]
pub struct Snapshot {
    pub pairs: Vec<Pair>,
    saved: Vec<Patch>,
    mode: u8,
    power_mode: u8,
    bitmap: u8,
}

trait RpmIo: SettingsAccess {
    fn fan_info(&self) -> Result<[u8; 256]>;
    fn exchange(&self, packet: &[u8; 256]) -> Result<[u8; 256]>;
    fn mode(&self, mode: u8) -> Result<()>;
}

impl RpmIo for Dchu {
    fn fan_info(&self) -> Result<[u8; 256]> {
        self.read_package(13)
    }
    fn exchange(&self, packet: &[u8; 256]) -> Result<[u8; 256]> {
        self.exchange_package(4, packet)
    }
    fn mode(&self, mode: u8) -> Result<()> {
        self.set_fan_mode_verified(mode)
    }
}

pub fn supported(capabilities: &[u8]) -> bool {
    capabilities.len() >= 21 && capabilities[..2] == [1, 0] && capabilities[20] & 0x10 != 0
}

fn read_byte(io: &impl SettingsAccess, page: u8, offset: usize) -> Result<u8> {
    let data = io.read(page, offset, 1).map_err(|e| anyhow!(e))?;
    if data.len() != 1 {
        bail!("OEM 字段长度异常");
    }
    Ok(data[0])
}

fn capture(io: &impl RpmIo) -> Result<Snapshot> {
    let capability = io.read(7, 0, 21).map_err(|e| anyhow!(e))?;
    if !supported(&capability) {
        bail!("固件未声明十级 RPM 风扇控制能力");
    }
    let info = io.fan_info()?;
    let bitmap = info[50] & 15;
    if !(1..=4).contains(&info[12]) || bitmap == 0 || info[43] & 2 != 0 {
        bail!("RPM 风扇数量、存在位或自定义能力无效");
    }
    let mode = decode_oem_fan_mode(info[14]).map_err(anyhow::Error::msg)?;
    let power_mode = read_byte(io, 1, 1)?;
    if power_mode > 4 {
        bail!("未知 OEM 电源模式，不能生成 RPM 表 ID");
    }
    let mut pairs = Vec::new();
    for (query, mask) in [(40, 3), (42, 12)] {
        if bitmap & mask == 0 {
            continue;
        }
        let mut request = [0; 256];
        request[0] = query;
        let pair = Pair::read(query, &io.exchange(&request)?).map_err(|e| anyhow!(e))?;
        for slot in 0..2 {
            if bitmap & (1 << pair.fan_index(slot)) != 0 {
                rpm::decode_curve(&pair.tables[slot]).map_err(|e| anyhow!(e))?;
            }
        }
        pairs.push(pair);
    }
    let saved = crate::fanctl::saved::mode(io, mode)?
        .iter()
        .map(|field| {
            io.read(4, field.offset, field.data.len())
                .map(|data| Patch::new(field.offset, data))
                .map_err(|e| anyhow!(e))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(Snapshot {
        pairs,
        saved,
        mode,
        power_mode,
        bitmap,
    })
}

pub fn checkpoint(hal: &Hal) -> Result<Snapshot> {
    hal.with_dchu(capture)
}

pub fn read(hal: &Hal) -> Result<Vec<RpmFan>> {
    let snapshot = checkpoint(hal)?;
    let mut fans = Vec::new();
    for pair in &snapshot.pairs {
        for slot in 0..2 {
            let index = pair.fan_index(slot);
            if snapshot.bitmap & (1 << index) == 0 {
                continue;
            }
            let table = &pair.tables[slot];
            fans.push(RpmFan {
                id: rpm::FAN_IDS[index].into(),
                maximum_rpm: rpm::maximum_rpm(table).map_err(|e| anyhow!(e))?,
                points: rpm::decode_curve(table).map_err(|e| anyhow!(e))?,
            });
        }
    }
    Ok(fans)
}

fn prepare(snapshot: &Snapshot, wanted: &Curves) -> Result<Vec<Pair>> {
    if wanted.is_empty() {
        bail!("没有提供 RPM 曲线");
    }
    for (id, points) in wanted {
        let index = rpm::FAN_IDS
            .iter()
            .position(|known| known == id)
            .ok_or_else(|| anyhow!("未知 RPM 风扇 {id}"))?;
        if snapshot.bitmap & (1 << index) == 0 {
            bail!("{id} 风扇不存在");
        }
        rpm::validate_curve(points).map_err(|e| anyhow!("{id}：{e}"))?;
    }
    let mut targets = Vec::new();
    for before in &snapshot.pairs {
        let mut pair = before.clone();
        for slot in 0..2 {
            let index = pair.fan_index(slot);
            if let Some(points) = wanted.get(rpm::FAN_IDS[index]) {
                pair.tables[slot] =
                    rpm::encode_curve(&pair.tables[slot], index, snapshot.power_mode, points)
                        .map_err(|e| anyhow!(e))?;
            }
        }
        if pair != *before {
            targets.push(pair);
        }
    }
    Ok(targets)
}

pub fn validate(hal: &Hal, wanted: &Curves) -> Result<()> {
    prepare(&checkpoint(hal)?, wanted).map(|_| ())
}

pub fn apply(hal: &Hal, wanted: &Curves) -> Result<()> {
    super::ensure_manual_allowed(hal)?;
    crate::powerctl::mode::boost::ensure_fan_allowed(
        hal,
        crate::powerctl::mode::boost::FanTarget::Firmware(super::FAN_MODE_CUSTOM),
    )?;
    let before = checkpoint(hal)?;
    let targets = prepare(&before, wanted)?;
    commit(hal, before, targets, 6)
}

pub fn restore_default(hal: &Hal) -> Result<()> {
    super::ensure_manual_allowed(hal)?;
    crate::powerctl::mode::boost::ensure_fan_allowed(
        hal,
        crate::powerctl::mode::boost::FanTarget::Firmware(super::FAN_MODE_CUSTOM),
    )?;
    let before = checkpoint(hal)?;
    let mut targets = before.pairs.clone();
    for pair in &mut targets {
        for slot in 0..2 {
            let index = pair.fan_index(slot);
            if before.bitmap & (1 << index) != 0 {
                pair.tables[slot] =
                    rpm::encode_default(&pair.tables[slot], index, before.power_mode)
                        .map_err(|e| anyhow!(e))?;
            }
        }
    }
    commit(hal, before, targets, 0)
}

fn commit(hal: &Hal, before: Snapshot, targets: Vec<Pair>, mode: u8) -> Result<()> {
    let saved = super::checkpoint(hal, false)?.with_rpm_tables(before.pairs.clone());
    anyhow::ensure!(saved.mode() == before.mode, "EC 风扇档位已变化，请重新读取");
    super::software::stop_safely(hal)?;
    super::checkpoint::remember_baseline(hal, &saved)?;
    hal.with_dchu(|io| transaction::apply(io, &before, &targets, mode))
}

pub fn restore_pairs(io: &Dchu, pairs: &[Pair]) -> Result<()> {
    transaction::restore_pairs(io, pairs)
}
