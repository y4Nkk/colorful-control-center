//! 原厂 GPU 锁频的 NVAPI V2 表与基础频率 V3 布局。
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

pub const GET_LOCKS: u32 = 0xE440_B867;
pub const SET_LOCKS: u32 = 0x3944_2CFB;
pub const GET_FREQUENCIES: u32 = 0xDCB6_16C3;
pub const GET_PCI_IDENTIFIERS: u32 = 0x2DDF_B66E;
pub const GET_BUS_ID: u32 = 0x1BE0_B8E5;
pub const GET_BUS_SLOT_ID: u32 = 0x2A0A_350F;
const LOCK_VERSION: u32 = 0x0002_030C;
const FREQUENCY_VERSION: u32 = 0x0003_0108;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LockEntry {
    pub domain: u32,
    pub unknown1: u32,
    pub mode: u32,
    pub unknown2: u32,
    pub value: u32,
    pub unknown3: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LockTable {
    pub version: u32,
    pub unknown: u32,
    pub count: u32,
    pub entries: [LockEntry; 32],
}
impl Default for LockTable {
    fn default() -> Self {
        Self {
            version: LOCK_VERSION,
            unknown: 0,
            count: 0,
            entries: [LockEntry::default(); 32],
        }
    }
}
impl LockTable {
    pub fn owned(&self) -> Result<[LockEntry; 2]> {
        if self.version != LOCK_VERSION || self.count as usize > self.entries.len() {
            bail!("GPU 锁频表版本或数量无效");
        }
        let mut found = [None, None];
        for entry in &self.entries[..self.count as usize] {
            if entry.domain < 2 {
                let slot = &mut found[entry.domain as usize];
                if slot.replace(*entry).is_some() {
                    bail!("GPU 锁频表包含重复的目标域");
                }
            }
        }
        match found {
            [Some(graphics), Some(other)] => Ok([graphics, other]),
            _ => bail!("GPU 锁频表缺少原厂所需的两个域"),
        }
    }

    #[cfg(test)]
    pub fn with_owned(&self, owned: [LockEntry; 2]) -> Result<Self> {
        self.owned()?;
        if owned[0].domain != 0 || owned[1].domain != 1 {
            bail!("GPU 锁频恢复域无效");
        }
        let mut target = *self;
        for entry in &mut target.entries[..target.count as usize] {
            if entry.domain < 2 {
                *entry = owned[entry.domain as usize];
            }
        }
        Ok(target)
    }

    pub fn request_owned(owned: [LockEntry; 2]) -> Result<Self> {
        if owned[0].domain != 0 || owned[1].domain != 1 {
            bail!("GPU 锁频请求域无效");
        }
        let mut request = Self {
            count: 2,
            ..Default::default()
        };
        request.entries[..2].copy_from_slice(&owned);
        Ok(request)
    }

    pub fn original_target(&self, mhz: u32) -> Result<Self> {
        let khz = mhz
            .checked_mul(1000)
            .ok_or_else(|| anyhow::anyhow!("GPU 锁频超出编码范围"))?;
        let graphics = LockEntry {
            domain: 0,
            mode: if mhz == 0 { 0 } else { 2 },
            value: khz,
            ..LockEntry::default()
        };
        let other = LockEntry {
            domain: 1,
            ..LockEntry::default()
        };
        self.owned()?;
        Self::request_owned([graphics, other])
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct ClockDomain {
    pub flags: u32,
    pub frequency_khz: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Frequencies {
    pub version: u32,
    pub flags: u32,
    pub domains: [ClockDomain; 32],
}
impl Default for Frequencies {
    fn default() -> Self {
        Self {
            version: FREQUENCY_VERSION,
            flags: 1,
            domains: [ClockDomain::default(); 32],
        }
    }
}
impl Frequencies {
    pub fn graphics_base_mhz(&self) -> Result<u32> {
        let graphics = self.domains[0];
        if self.version != FREQUENCY_VERSION || self.flags & 0xF != 1 || graphics.flags & 1 == 0 {
            bail!("GPU 基础图形频率结构无效或该域不可用");
        }
        let mhz = graphics.frequency_khz / 1000;
        if mhz == 0 {
            bail!("GPU 基础图形频率未报告");
        }
        Ok(mhz)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{offset_of, size_of};

    fn table() -> LockTable {
        let mut table = LockTable {
            unknown: 7,
            count: 3,
            ..Default::default()
        };
        table.entries[0] = LockEntry {
            domain: 5,
            mode: 3,
            value: 600,
            unknown1: 9,
            ..Default::default()
        };
        table.entries[1] = LockEntry {
            domain: 1,
            mode: 3,
            value: 800,
            unknown3: 8,
            ..Default::default()
        };
        table.entries[2] = LockEntry {
            domain: 0,
            mode: 3,
            value: 900,
            unknown2: 6,
            ..Default::default()
        };
        table
    }

    #[test]
    fn original_abi_has_exact_sizes_versions_and_offsets() {
        assert_eq!(size_of::<LockEntry>(), 24);
        assert_eq!(size_of::<LockTable>(), 780);
        assert_eq!(offset_of!(LockTable, entries), 12);
        assert_eq!(offset_of!(LockEntry, mode), 8);
        assert_eq!(offset_of!(LockEntry, value), 16);
        assert_eq!(size_of::<Frequencies>(), 264);
        assert_eq!(offset_of!(Frequencies, domains), 8);
        assert_eq!(LockTable::default().version, 0x2030c);
        assert_eq!(Frequencies::default().version, 0x30108);
    }

    #[test]
    fn target_matches_original_two_domains_and_preserves_unowned_fields() {
        let before = table();
        let target = before.original_target(1750).unwrap();
        assert_eq!(target.unknown, 0);
        assert_eq!(target.count, 2);
        assert_eq!(target.entries[2], LockEntry::default());
        assert_eq!(
            target.owned().unwrap(),
            [
                LockEntry {
                    mode: 2,
                    value: 1_750_000,
                    ..Default::default()
                },
                LockEntry {
                    domain: 1,
                    ..Default::default()
                }
            ]
        );
        let reset = before.original_target(0).unwrap().owned().unwrap();
        assert_eq!(
            reset,
            [
                LockEntry::default(),
                LockEntry {
                    domain: 1,
                    ..Default::default()
                }
            ]
        );
        let changed = before.with_owned(target.owned().unwrap()).unwrap();
        assert_eq!(changed.entries[0], before.entries[0]);
        assert_eq!(changed.with_owned(before.owned().unwrap()).unwrap(), before);
        assert!(before.original_target(u32::MAX).is_err());
    }

    #[test]
    fn malformed_duplicate_or_incomplete_lock_tables_are_rejected() {
        let mut invalid = table();
        invalid.count = 33;
        assert!(invalid.owned().is_err());
        invalid = table();
        invalid.version = 1;
        assert!(invalid.owned().is_err());
        invalid = table();
        invalid.entries[0].domain = 0;
        assert!(invalid.owned().is_err());
        invalid = table();
        invalid.count = 2;
        assert!(invalid.owned().is_err());
    }

    #[test]
    fn base_clock_requires_v3_base_type_present_graphics_and_nonzero_mhz() {
        let mut frequencies = Frequencies::default();
        assert!(frequencies.graphics_base_mhz().is_err());
        frequencies.domains[0] = ClockDomain {
            flags: 1,
            frequency_khz: 1_350_999,
        };
        assert_eq!(frequencies.graphics_base_mhz().unwrap(), 1350);
        frequencies.flags = 0;
        assert!(frequencies.graphics_base_mhz().is_err());
        frequencies.flags = 1;
        frequencies.domains[0].frequency_khz = 999;
        assert!(frequencies.graphics_base_mhz().is_err());
    }
}
