use super::{Structure, firmware_table, structure_at, structure_table};
use anyhow::{Context, Result, ensure};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MemoryModule {
    locator: Option<String>,
    capacity_bytes: Option<u64>,
    memory_type: Option<&'static str>,
    manufacturer: Option<String>,
    part_number: Option<String>,
    configured_speed_mts: Option<u32>,
    soldered: Option<bool>,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
pub(crate) struct MemorySlots {
    used: usize,
    total: usize,
}

pub(in crate::hal) struct MemoryInventory {
    pub(in crate::hal) modules: Vec<MemoryModule>,
    pub(in crate::hal) slots: Option<MemorySlots>,
    pub(in crate::hal) speed_mts: Option<u32>,
}

pub(in crate::hal) fn read_inventory() -> Result<MemoryInventory> {
    inventory(&firmware_table()?)
}

struct MemoryDevice {
    present: Option<bool>,
    socketed: Option<bool>,
    module: MemoryModule,
}

fn inventory(raw: &[u8]) -> Result<MemoryInventory> {
    let table = structure_table(raw)?;
    let mut arrays = BTreeMap::new();
    let mut devices = Vec::new();
    let mut at = 0;
    for _ in 0..4096 {
        let Some((structure, next)) = structure_at(table, at)? else {
            break;
        };
        at = next;
        let data = structure.formatted;
        match structure.kind {
            16 if data.get(5) == Some(&3) => {
                let handle = word(data, 2).context("系统内存阵列句柄缺失")?;
                ensure!(
                    arrays.insert(handle, word(data, 0x0d)).is_none(),
                    "系统内存阵列句柄重复"
                );
            }
            17 => devices.push(structure),
            127 => {
                ensure!(
                    table[at..].iter().all(|byte| *byte == 0),
                    "SMBIOS 结束结构后仍有未知数据"
                );
                at = table.len();
                break;
            }
            _ => {}
        }
    }
    ensure!(at == table.len(), "SMBIOS 结构数量超过支持范围");
    ensure!(!arrays.is_empty(), "SMBIOS 缺少系统内存阵列");
    let mut counts = BTreeMap::<u16, usize>::new();
    let mut handles = BTreeSet::new();
    let mut physical = Vec::new();
    let mut unique = true;
    for structure in devices {
        let data = structure.formatted;
        let Some(array) = word(data, 4).filter(|handle| arrays.contains_key(handle)) else {
            continue;
        };
        let handle = word(data, 2).context("内存设备句柄缺失")?;
        if !handles.insert(handle) {
            unique = false;
            continue;
        }
        *counts.entry(array).or_default() += 1;
        if data.get(0x12) != Some(&0x1f) {
            physical.push(decode_device(&structure));
        }
    }
    let complete = unique
        && arrays.iter().all(|(handle, total)| {
            total.is_some_and(|total| {
                total != 0 && Some(usize::from(total)) == counts.get(handle).copied()
            })
        });
    let slots = complete.then(|| physical_slots(&physical)).flatten();
    let speed_mts = complete.then(|| common_speed(&physical)).flatten();
    Ok(MemoryInventory {
        modules: physical
            .into_iter()
            .filter_map(|device| (device.present != Some(false)).then_some(device.module))
            .collect(),
        slots,
        speed_mts,
    })
}

fn decode_device(structure: &Structure<'_>) -> MemoryDevice {
    let data = structure.formatted;
    let size = word(data, 0x0c);
    let socketed = match data.get(0x0e) {
        Some(0x03 | 0x09 | 0x0c | 0x0d | 0x0f) => Some(true),
        Some(0x05 | 0x0b | 0x10) => Some(false),
        _ => None,
    };
    MemoryDevice {
        present: size.map(|value| value != 0),
        socketed,
        module: MemoryModule {
            locator: string_at(structure, 0x10),
            capacity_bytes: capacity(size, data),
            memory_type: match data.get(0x12) {
                Some(0x12) => Some("DDR"),
                Some(0x13) => Some("DDR2"),
                Some(0x18) => Some("DDR3"),
                Some(0x1a) => Some("DDR4"),
                Some(0x22) => Some("DDR5"),
                Some(0x1b) => Some("LPDDR"),
                Some(0x1c) => Some("LPDDR2"),
                Some(0x1d) => Some("LPDDR3"),
                Some(0x1e) => Some("LPDDR4"),
                Some(0x23) => Some("LPDDR5"),
                _ => None,
            },
            manufacturer: string_at(structure, 0x17),
            part_number: string_at(structure, 0x1a),
            configured_speed_mts: match word(data, 0x20) {
                Some(0xffff) => {
                    dword(data, 0x58).filter(|value| (0xffff..=0x7fff_ffff).contains(value))
                }
                Some(value) if value != 0 => Some(u32::from(value)),
                _ => None,
            },
            soldered: socketed.map(|value| !value),
        },
    }
}

fn physical_slots(devices: &[MemoryDevice]) -> Option<MemorySlots> {
    let mut slots = MemorySlots { used: 0, total: 0 };
    for device in devices {
        if device.socketed? {
            slots.total += 1;
            slots.used += usize::from(device.present?);
        }
    }
    (slots.total != 0).then_some(slots)
}

fn common_speed(devices: &[MemoryDevice]) -> Option<u32> {
    let mut common = None;
    for device in devices {
        if device.present? {
            let speed = device.module.configured_speed_mts?;
            if common.is_some_and(|current| current != speed) {
                return None;
            }
            common = Some(speed);
        }
    }
    common
}

fn capacity(size: Option<u16>, data: &[u8]) -> Option<u64> {
    match size? {
        0 | 0xffff | 0x8000 => None,
        0x7fff => dword(data, 0x1c)
            .filter(|value| (0x7fff..=0x7fff_ffff).contains(value))
            .map(|value| u64::from(value) << 20),
        value if value & 0x8000 != 0 => Some(u64::from(value & 0x7fff) << 10),
        value => Some(u64::from(value) << 20),
    }
}

fn word(data: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        data.get(offset..offset + 2)?.try_into().ok()?,
    ))
}

fn dword(data: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        data.get(offset..offset + 4)?.try_into().ok()?,
    ))
}

fn string_at(structure: &Structure<'_>, offset: usize) -> Option<String> {
    let index = usize::from(*structure.formatted.get(offset)?).checked_sub(1)?;
    let bytes = structure.strings.split(|byte| *byte == 0).nth(index)?;
    let value = std::str::from_utf8(bytes).ok()?.trim();
    (!value.is_empty() && value.len() <= 4096).then(|| value.to_owned())
}

#[cfg(test)]
mod tests;
